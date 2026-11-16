use super::*;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const TEST_TIMEOUT: Duration = Duration::from_secs(5);
const ISSUE_URL: &str = "https://example.invalid/issues/1";
const SUCCESS_RESPONSE: &str = r#"{"issueUrl":"https://example.invalid/issues/1"}"#;

struct MockHttp {
    endpoint: String,
    request: JoinHandle<(String, Value)>,
}

impl MockHttp {
    fn new(status: u16, body: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/report", listener.local_addr().unwrap());
        let body = body.to_owned();
        let request = thread::spawn(move || {
            let deadline = Instant::now() + TEST_TIMEOUT;
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "mock server never received a request"
                        );
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("mock server accept failed: {error}"),
                }
            };
            stream.set_write_timeout(Some(TEST_TIMEOUT)).unwrap();
            let request = read_request(&mut stream);
            write!(
                stream,
                "HTTP/1.1 {status} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            request
        });
        Self { endpoint, request }
    }

    fn finish(self) -> (String, Value) {
        self.request.join().expect("mock HTTP server failed")
    }
}

fn read_request(stream: &mut TcpStream) -> (String, Value) {
    let deadline = Instant::now() + TEST_TIMEOUT;
    let mut bytes = Vec::new();
    let mut header_end = None;
    let mut content_length = None;
    loop {
        if let (Some(end), Some(length)) = (header_end, content_length) {
            if bytes.len() >= end + length {
                assert_eq!(
                    bytes.len(),
                    end + length,
                    "unexpected trailing request bytes"
                );
                let headers = String::from_utf8(bytes[..end].to_vec()).unwrap();
                let payload = serde_json::from_slice(&bytes[end..]).expect("request body is JSON");
                return (headers, payload);
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "timed out reading complete HTTP request"
        );
        stream.set_read_timeout(Some(remaining)).unwrap();
        let mut chunk = [0; 4096];
        let count = stream.read(&mut chunk).expect("read mock HTTP request");
        assert_ne!(
            count, 0,
            "client closed before sending complete HTTP request"
        );
        bytes.extend_from_slice(&chunk[..count]);
        assert!(bytes.len() <= 1_000_000, "unexpectedly large request");
        if header_end.is_none() {
            if let Some(pos) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let end = pos + 4;
                let headers = std::str::from_utf8(&bytes[..end]).unwrap();
                let length = headers
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                    .expect("JSON request must include Content-Length")
                    .1
                    .trim()
                    .parse::<usize>()
                    .unwrap();
                assert!(length <= 900_000, "unexpectedly large Content-Length");
                header_end = Some(end);
                content_length = Some(length);
            } else {
                assert!(bytes.len() <= 16_384, "request headers exceed test limit");
            }
        }
    }
}

fn widget_for(server: &MockHttp) -> BugReportWidget {
    let mut widget = BugReportWidget::new("/home/test");
    widget.report_endpoint = server.endpoint.clone();
    widget.open();
    widget.summary = "Editor stops responding".into();
    widget
}

fn wait_for_submission(widget: &mut BugReportWidget) {
    let deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        if widget.poll_submission() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "submission never completed: {:?}",
            widget.stage
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert!(widget.submission.is_none());
    assert!(
        !widget.poll_submission(),
        "completion must be reported only once"
    );
}

#[test]
fn redaction_terminates_for_repeated_empty_and_unicode_secrets() {
    let widget = BugReportWidget::new("/home/test");
    for (input, expected) in [
        ("ghp_abc ghp_def", "ghp_[REDACTED] ghp_[REDACTED]"),
        (
            "token= token=abc&password=\u{79d8}\u{5bc6}",
            "token= token=[REDACTED]&password=[REDACTED]",
        ),
        ("token=", "token="),
        ("sk-[REDACTED]", "sk-[REDACTED]"),
        (
            "/home/test/\u{65e5}\u{672c}\u{8a9e} api_key=\u{9375}",
            "~/\u{65e5}\u{672c}\u{8a9e} api_key=[REDACTED]",
        ),
    ] {
        assert_eq!(widget.redact_text(input), expected);
        assert_eq!(widget.redact_text(expected), expected);
    }
}

#[test]
fn redacts_all_secret_prefixes_and_preserves_delimiters() {
    let widget = BugReportWidget::new("");
    for prefix in [
        "ghp_", "github_pat_", "gho_", "ghu_", "ghs_", "ghr_", "sk-", "Bearer ", "token=",
        "api_key=", "apikey=", "secret=", "password=",
    ] {
        for delimiter in [" ", "\n", "\"", "'", "&", ""] {
            let input = format!("before {prefix}sensitive{delimiter}");
            let expected = format!("before {prefix}[REDACTED]{delimiter}");
            assert_eq!(widget.redact_text(&input), expected);
        }
    }
    assert_eq!(widget.redact_text("/home/test/project"), "/home/test/project");
}

#[test]
fn success_posts_git_category_and_redacted_serialized_diagnostics() {
    let server = MockHttp::new(201, SUCCESS_RESPONSE);
    let mut widget = widget_for(&server);
    widget.category_idx = BUG_CATEGORIES
        .iter()
        .position(|category| *category == "Git")
        .unwrap();
    widget.summary = "Git fails at /home/test/repo token=summary-secret".into();
    widget.description = "Run in /home/test/repo\nBearer description-secret".into();
    let diagnostics = json!({
        "cwd": "/home/test/repo",
        "credential": "ghp_diagnostics-secret",
        "message": "a quoted \"value\" and newline\nnext line",
    })
    .to_string();
    widget.error_message = "previous failure".into();
    widget.issue_url = "https://example.invalid/stale".into();

    widget.submit(&diagnostics);
    assert_eq!(widget.stage, BugStage::Submitting);
    assert!(widget.error_message.is_empty());
    assert!(widget.issue_url.is_empty());
    wait_for_submission(&mut widget);
    assert_eq!(widget.stage, BugStage::Success, "{}", widget.error_message);
    assert_eq!(widget.issue_url, ISSUE_URL);

    let (headers, payload) = server.finish();
    assert!(headers.starts_with("POST /report HTTP/1.1\r\n"));
    assert!(headers.lines().any(|line| line
        .to_ascii_lowercase()
        .starts_with("content-type: application/json")));
    assert_eq!(payload.as_object().unwrap().len(), 4);
    assert_eq!(payload["category"], "Git");
    assert_eq!(payload["summary"], "Git fails at ~/repo token=[REDACTED]");
    assert_eq!(payload["description"], "Run in ~/repo\nBearer [REDACTED]");
    let attached: Value = serde_json::from_str(payload["diagnostics"].as_str().unwrap()).unwrap();
    assert_eq!(
        attached,
        json!({
            "cwd": "~/repo",
            "credential": "ghp_[REDACTED]",
            "message": "a quoted \"value\" and newline\nnext line",
        })
    );
    let serialized = payload.to_string();
    for secret in [
        "/home/test",
        "summary-secret",
        "description-secret",
        "diagnostics-secret",
    ] {
        assert!(!serialized.contains(secret), "payload leaked {secret}");
    }
}

#[test]
fn description_is_optional_and_success_can_be_reopened_as_a_fresh_form() {
    let server = MockHttp::new(200, SUCCESS_RESPONSE);
    let mut widget = widget_for(&server);
    assert!(widget.description.is_empty());
    widget.submit("{}");
    wait_for_submission(&mut widget);
    assert_eq!(widget.stage, BugStage::Success, "{}", widget.error_message);
    let (_, payload) = server.finish();
    assert_eq!(payload["description"], "");

    assert!(widget.handle_key(&InputEvent::Key("<CR>".into()), "{}"));
    assert!(!widget.is_open);
    widget.open();
    assert!(widget.is_open);
    assert_eq!(widget.stage, BugStage::Form);
    assert!(widget.summary.is_empty());
    assert!(widget.description.is_empty());
    assert!(widget.issue_url.is_empty());
}

#[test]
fn submitted_fields_are_redacted_then_bounded_by_unicode_character_count() {
    let server = MockHttp::new(201, SUCCESS_RESPONSE);
    let mut widget = widget_for(&server);
    widget.summary = format!("/home/test{}", "\u{e9}".repeat(200));
    widget.description = "\u{e9}".repeat(6100);
    let diagnostics = "\u{65e5}".repeat(40_100);
    let summary = widget.summary.clone();
    let description = widget.description.clone();
    widget.submit(&diagnostics);
    wait_for_submission(&mut widget);
    assert_eq!(widget.stage, BugStage::Success, "{}", widget.error_message);
    let (_, payload) = server.finish();
    assert_eq!(payload["summary"], format!("~{}", "\u{e9}".repeat(119)));
    assert_eq!(payload["description"], "\u{e9}".repeat(6000));
    assert_eq!(payload["diagnostics"], "\u{65e5}".repeat(40_000));
    assert_eq!(
        widget.summary, summary,
        "submission must not mutate the draft"
    );
    assert_eq!(widget.description, description);
}

#[test]
fn http_failures_have_actionable_messages_and_preserve_the_draft() {
    for (status, expected_message) in [
        (400, "rejected"),
        (413, "rejected"),
        (422, "rejected"),
        (429, "Too many"),
        (500, "500"),
        (503, "503"),
    ] {
        let server = MockHttp::new(status, SUCCESS_RESPONSE);
        let mut widget = widget_for(&server);
        widget.description = "Steps to reproduce\nDo not discard this".into();
        widget.category_idx = 2;
        let summary = widget.summary.clone();
        let description = widget.description.clone();
        widget.submit("{}");
        wait_for_submission(&mut widget);
        server.finish();
        assert_eq!(widget.stage, BugStage::Failure, "HTTP {status}");
        assert!(
            widget.error_message.contains(expected_message),
            "HTTP {status}: {}",
            widget.error_message
        );
        assert!(widget.issue_url.is_empty());
        assert_eq!(widget.summary, summary);
        assert_eq!(widget.description, description);
        assert_eq!(widget.category_idx, 2);
    }
}

#[test]
fn successful_http_status_still_requires_valid_json_and_an_https_issue_url() {
    for body in [
        "not json",
        "{}",
        r#"{"issueUrl":null}"#,
        r#"{"issueUrl":42}"#,
        r#"{"issueUrl":""}"#,
        r#"{"issueUrl":"http://example.invalid/issues/1"}"#,
        r#"{"issueUrl":"javascript:alert(1)"}"#,
    ] {
        let server = MockHttp::new(200, body);
        let mut widget = widget_for(&server);
        widget.submit("{}");
        wait_for_submission(&mut widget);
        server.finish();
        assert_eq!(widget.stage, BugStage::Failure, "response: {body}");
        assert!(
            widget.error_message.contains("invalid response"),
            "{}",
            widget.error_message
        );
        assert!(widget.issue_url.is_empty());
    }
}

#[test]
fn enter_after_failure_returns_to_editing_and_allows_retry() {
    let failed_server = MockHttp::new(422, "{}");
    let mut widget = widget_for(&failed_server);
    widget.description = "Original reproduction steps".into();
    let summary = widget.summary.clone();
    widget.submit("{}");
    wait_for_submission(&mut widget);
    failed_server.finish();
    assert_eq!(widget.stage, BugStage::Failure);

    assert!(widget.handle_key(&InputEvent::Key("<CR>".into()), "{}"));
    assert!(widget.is_open);
    assert_eq!(widget.stage, BugStage::Form);
    assert_eq!(widget.summary, summary);
    assert_eq!(widget.description, "Original reproduction steps");
    widget.focus = 1;
    assert!(widget.handle_key(&InputEvent::Key("!".into()), "{}"));
    assert_eq!(widget.summary, format!("{summary}!"));

    let retry_server = MockHttp::new(201, SUCCESS_RESPONSE);
    widget.report_endpoint = retry_server.endpoint.clone();
    assert!(widget.handle_key(&InputEvent::Key("<C-s>".into()), "{}"));
    assert_eq!(widget.stage, BugStage::Submitting);
    wait_for_submission(&mut widget);
    assert_eq!(widget.stage, BugStage::Success, "{}", widget.error_message);
    assert!(widget.error_message.is_empty());
    let (_, payload) = retry_server.finish();
    assert_eq!(payload["summary"], format!("{summary}!"));
    assert_eq!(payload["description"], "Original reproduction steps");
}

#[test]
fn blank_summary_is_rejected_without_starting_an_upload() {
    for summary in ["", " \t\r\n"] {
        for action in ["submit", "<C-s>", "<CR>"] {
            let mut widget = BugReportWidget::new("/home/test");
            widget.report_endpoint = "http://127.0.0.1:0/report".into();
            widget.open();
            widget.summary = summary.into();
            widget.focus = 3;
            if action == "submit" {
                widget.submit("{}");
            } else {
                assert!(widget.handle_key(&InputEvent::Key(action.into()), "{}"));
            }
            assert_eq!(widget.stage, BugStage::Form, "action: {action}");
            assert!(widget.submission.is_none());
            assert!(
                !widget.error_message.is_empty(),
                "validation missing for {action}"
            );
            assert_eq!(widget.focus, 1);
            assert_eq!(widget.summary, summary);
            assert!(!widget.poll_submission());
        }
    }
}

#[test]
fn ctrl_s_and_send_button_submit_directly_without_a_consent_step() {
    for key in ["<C-s>", "<CR>"] {
        let server = MockHttp::new(201, SUCCESS_RESPONSE);
        let mut widget = widget_for(&server);
        widget.focus = 3;
        assert!(widget.handle_key(
            &InputEvent::Key(key.into()),
            r#"{"source":"keyboard"}"#
        ));
        assert_eq!(widget.stage, BugStage::Submitting, "key: {key}");
        assert!(widget.submission.is_some());
        wait_for_submission(&mut widget);
        assert_eq!(widget.stage, BugStage::Success, "{}", widget.error_message);
        let (_, payload) = server.finish();
        assert_eq!(payload["diagnostics"], r#"{"source":"keyboard"}"#);
    }
}

#[test]
fn pending_submission_survives_duplicate_send_close_and_reopen() {
    let mut widget = BugReportWidget::new("/home/test");
    widget.report_endpoint = "http://127.0.0.1:0/report".into();
    widget.open();
    widget.summary = "Keep the pending report".into();
    widget.description = "Keep its draft".into();
    let (sender, receiver) = mpsc::channel();
    widget.submission = Some(receiver);
    widget.stage = BugStage::Submitting;

    assert!(!widget.poll_submission(), "an empty channel is still pending");
    widget.submit("duplicate diagnostics");
    assert!(widget.handle_key(&InputEvent::Key("<C-s>".into()), "{}"));
    assert!(widget.handle_key(&InputEvent::Key("<CR>".into()), "{}"));
    assert!(widget.handle_key(&InputEvent::Key("<Esc>".into()), "{}"));
    assert!(!widget.is_open);
    widget.open();
    assert!(widget.is_open);
    assert_eq!(widget.stage, BugStage::Submitting);
    assert_eq!(widget.summary, "Keep the pending report");
    assert_eq!(widget.description, "Keep its draft");

    // Completing the original channel proves duplicate sends did not replace it.
    sender.send(Ok(ISSUE_URL.into())).unwrap();
    assert!(widget.poll_submission());
    assert_eq!(widget.stage, BugStage::Success);
    assert_eq!(widget.issue_url, ISSUE_URL);
    assert!(widget.submission.is_none());
    assert!(!widget.poll_submission());
}

#[test]
fn disconnected_submission_channel_reports_failure_and_allows_editing() {
    let mut widget = BugReportWidget::new("/home/test");
    widget.open();
    widget.summary = "Do not lose this draft".into();
    let (sender, receiver) = mpsc::channel();
    widget.submission = Some(receiver);
    widget.stage = BugStage::Submitting;
    drop(sender);

    assert!(widget.poll_submission());
    assert_eq!(widget.stage, BugStage::Failure);
    assert!(!widget.error_message.is_empty());
    assert!(widget.submission.is_none());
    assert!(!widget.poll_submission());
    assert_eq!(widget.summary, "Do not lose this draft");
    assert!(widget.handle_key(&InputEvent::Key("<CR>".into()), "{}"));
    assert!(widget.is_open);
    assert_eq!(widget.stage, BugStage::Form);
    assert_eq!(widget.summary, "Do not lose this draft");
}

#[test]
fn mouse_focuses_fields_and_send_uses_the_native_uploader() {
    let server = MockHttp::new(201, SUCCESS_RESPONSE);
    let mut widget = widget_for(&server);
    let screen = Rect::new(0, 0, 100, 30);
    let mr = BugReportWidget::modal_rect(screen);
    widget.handle_mouse(screen, mr.x + 14, mr.y + 2, 0, true, "{}");
    assert_eq!(widget.category_idx, 1);
    widget.handle_mouse(screen, mr.x + 2, mr.y + 8, 0, true, "{}");
    assert_eq!(widget.focus, 2);
    widget.handle_mouse(screen, mr.x + 2, mr.y + 5, 0, true, "{}");
    assert_eq!(widget.focus, 1);

    let (x, y) = (mr.x + mr.w - 10, mr.y + mr.h - 2);
    widget.handle_mouse(screen, x, y, 0, false, "{}");
    widget.handle_mouse(screen, x, y, 2, true, "{}");
    assert!(widget.submission.is_none());
    widget.handle_mouse(screen, x, y, 0, true, "{}");
    assert_eq!(widget.stage, BugStage::Submitting);
    wait_for_submission(&mut widget);
    assert_eq!(widget.stage, BugStage::Success);
    server.finish();
}

#[test]
fn public_report_notice_is_visible_in_small_and_large_terminals() {
    for (w, h) in [(42, 16), (80, 24), (120, 40)] {
        let mut widget = BugReportWidget::new("/home/test");
        widget.open();
        let mut renderer = Renderer::new(w, h);
        widget.draw(&mut renderer, Rect::new(0, 0, w, h), &Theme::default());
        let text: String = renderer.buf.iter()
            .map(|cell| std::str::from_utf8(&cell.ch[..cell.len as usize]).unwrap())
            .collect();
        for phrase in ["Public GitHub issue", "basic diagnostics", "No files or logs", "Send"] {
            assert!(text.contains(phrase), "missing {phrase} at {w}x{h}: {text}");
        }
    }
}
