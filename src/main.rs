use pulldown_cmark::{Options, Parser, html};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

const CSS: &str = r#"
<style>
body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
    font-size: 16px;
    line-height: 1.6;
    color: #e6edf3;
    background: #0d1117;
    max-width: 860px;
    margin: 0 auto;
    padding: 32px 24px;
}
h1, h2, h3, h4, h5, h6 { margin-top: 24px; margin-bottom: 16px; font-weight: 600; line-height: 1.25; color: #f0f6fc; }
h1 { font-size: 2em; padding-bottom: 0.3em; border-bottom: 1px solid #21262d; }
h2 { font-size: 1.5em; padding-bottom: 0.3em; border-bottom: 1px solid #21262d; }
h3 { font-size: 1.25em; }
code {
    background: #161b22;
    padding: 0.2em 0.4em;
    border-radius: 6px;
    font-size: 85%;
    font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
    color: #e6edf3;
}
pre {
    background: #161b22;
    padding: 16px;
    border-radius: 6px;
    overflow-x: auto;
    line-height: 1.45;
}
pre code { background: none; padding: 0; font-size: 85%; }
blockquote {
    margin: 0;
    padding: 0 1em;
    color: #8b949e;
    border-left: 0.25em solid #30363d;
}
table { border-collapse: collapse; width: 100%; margin: 16px 0; }
th, td { border: 1px solid #30363d; padding: 6px 13px; }
th { background: #161b22; font-weight: 600; }
tr:nth-child(even) { background: #161b22; }
a { color: #58a6ff; text-decoration: none; }
a:hover { text-decoration: underline; }
img { max-width: 100%; }
hr { border: none; border-top: 1px solid #21262d; margin: 24px 0; }
ul, ol { padding-left: 2em; }
li + li { margin-top: 0.25em; }
strong { font-weight: 600; color: #f0f6fc; }
</style>
"#;

fn render_markdown(md_content: &str) -> String {
    let options = Options::all();
    let parser = Parser::new_ext(md_content, options);
    let mut html_output = String::with_capacity(md_content.len() * 2);
    html::push_html(&mut html_output, parser);

    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\">{CSS}</head><body>{html_output}</body></html>"
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let md_content = if args.len() > 1 {
        let path = &args[1];
        match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => format!("# Error\n\nCould not read file: `{path}`\n\n```\n{e}\n```"),
        }
    } else {
        String::from("# ViewMD\n\nNo file specified.\n\nUsage: `viewmd <file.md>`")
    };

    let title = if args.len() > 1 {
        let path = std::path::Path::new(&args[1]);
        format!("{} — ViewMD", path.file_name().unwrap_or_default().to_string_lossy())
    } else {
        String::from("ViewMD")
    };

    let html = render_markdown(&md_content);

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(&title)
        .with_inner_size(tao::dpi::LogicalSize::new(920.0, 700.0))
        .build(&event_loop)
        .expect("Failed to create window");

    let _webview = WebViewBuilder::new()
        .with_html(&html)
        .build(&window)
        .expect("Failed to create webview");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}
