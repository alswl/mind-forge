use assert_cmd::Command;
use serde_json::Value;
use std::{fs, path::Path};
fn write(root: &Path, path: &str, text: &str) {
    let p = root.join(path);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}
fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("mf").unwrap().current_dir(root).arg("--json").args(args).output().unwrap()
}
fn ok(root: &Path, args: &[&str]) -> Value {
    let o = run(root, args);
    assert!(o.status.success(), "{args:?}: {}", String::from_utf8_lossy(&o.stderr));
    serde_json::from_slice::<Value>(&o.stdout).unwrap()["data"].clone()
}
fn tree(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut items = walkdir::WalkDir::new(root)
        .into_iter()
        .map(|e| e.unwrap())
        .map(|e| {
            (
                e.path().strip_prefix(root).unwrap().to_string_lossy().into_owned(),
                if e.file_type().is_file() { fs::read(e.path()).unwrap() } else { vec![] },
            )
        })
        .collect::<Vec<_>>();
    items.sort();
    items
}
fn setup(mode: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "minds.yaml", "schema: '1'\nprojects: ['.']\n");
    write(dir.path(), "mind.yaml", &format!("content_layout: {mode}\nminds:\n  brief: {{}}\n  empty: {{}}\n"));
    dir
}
const CONTENT: &str = "# Public title\n\n| Name | Value |\n| --- | --- |\n| Test | 42 |\n\n```rust\nlet value = 42;\n```\n\n```mermaid\ngraph TD\n  A[Start] --> B[End]\n```\n\n<script>window.documentExecuted = true</script>\n\n[unsafe](javascript:alert(1))\n\n> [!mf-private]\n> PRIVATE_SECRET_IN_CALLOUT\n\n## Public ending\n";
#[test]
fn both_layouts_shapes_outputs_and_private_separation() {
    for mode in ["category", "bundle"] {
        let dir = setup(mode);
        let root = dir.path();
        let base = if mode == "category" { "outputs/brief" } else { "brief/outputs" };
        write(root, &format!("{base}/single.md"), CONTENT);
        write(root, &format!("{base}/blocks/01.md"), CONTENT);
        write(
            root,
            &format!("{base}/blocks/02.md"),
            "---\nmind-forge-visibility: private\n---\nPRIVATE_SECRET_IN_BLOCK\n",
        );
        write(root, &format!("{base}/blocks/03.md"), "## Last block\nPUBLIC_LAST\n");
        write(root, "prompts/brief.md", "PROMPT_SECRET");
        write(root, "thinking/brief.md", "THINKING_SECRET");
        if mode == "bundle" {
            fs::remove_file(root.join("prompts/brief.md")).unwrap();
            fs::remove_dir(root.join("prompts")).unwrap();
            fs::remove_file(root.join("thinking/brief.md")).unwrap();
            fs::remove_dir(root.join("thinking")).unwrap();
            write(root, "brief/prompt.md", "PROMPT_SECRET");
            write(root, "brief/thinking.md", "THINKING_SECRET");
        }
        write(root, if mode == "category" { "reviews/brief.md" } else { "brief/reviews.md" }, "ADDITIONAL_SECRET");
        for name in ["single", "blocks"] {
            let identity = format!("::brief::{name}");
            let before = tree(root);
            let preview = ok(root, &["build", &identity, "--format", "html", "-n"]);
            assert_eq!(preview["format"], "html");
            assert!(preview["output_path"].as_str().unwrap().ends_with(&format!("build/brief/{name}.html")));
            assert_eq!(tree(root), before);
            let rendered = ok(root, &["build", &identity, "--format", "html"]);
            let html = fs::read_to_string(rendered["output"].as_str().unwrap()).unwrap();
            for token in [
                "PRIVATE_SECRET",
                "PROMPT_SECRET",
                "THINKING_SECRET",
                "ADDITIONAL_SECRET",
                "href=\"javascript:",
                "<script>window.documentExecuted",
            ] {
                assert!(!html.contains(token), "{token}");
            }
            for token in [
                "<table>",
                "language-rust",
                "mf-diagram",
                "&lt;script&gt;",
                "mermaid@11.12.0",
                "Content-Security-Policy",
            ] {
                assert!(html.contains(token), "{token}");
            }
            if name == "blocks" {
                assert!(html.contains("PUBLIC_LAST"));
            }
            let md = ok(root, &["build", &identity]);
            assert_eq!(md["format"], "markdown");
            assert!(md["output"].as_str().unwrap().ends_with(".md"));
            assert_eq!(fs::read_to_string(rendered["output"].as_str().unwrap()).unwrap(), html);
        }
        assert_eq!(fs::read_to_string(root.join(format!("{base}/single.md"))).unwrap(), CONTENT);
        let ambiguous = run(root, &["build", "::brief", "--format", "html"]);
        assert!(!ambiguous.status.success());
        assert!(String::from_utf8_lossy(&ambiguous.stderr).contains("selection_ambiguous"));
        assert!(!run(root, &["build", "::empty", "--format", "html"]).status.success());
    }
}
#[test]
fn template_override_is_static_and_missing_invalid_templates_do_not_write() {
    let dir = setup("category");
    let root = dir.path();
    write(root, "outputs/brief/main.md", CONTENT);
    write(
        root,
        "mind.yaml",
        "content_layout: category\nminds:\n  brief: {}\nbuild:\n  html_template: templates/page.html\n",
    );
    write(
        root,
        "templates/page.html",
        "<header class=\"custom\">PROJECT TEMPLATE {{title}}</header><main>{{content}}</main><script>window.templateExecuted=true</script>",
    );
    let data = ok(root, &["build", "::brief::main", "--format", "html"]);
    let html = fs::read_to_string(data["output"].as_str().unwrap()).unwrap();
    assert!(html.contains("PROJECT TEMPLATE ::brief::main"));
    assert!(!html.contains("window.templateExecuted"));
    for template in [
        "<main>missing token</main>",
        "<main>{{content}}{{content}}</main>",
        "<div title=\"{{content}}\">invalid attribute</div>",
    ] {
        write(root, "templates/page.html", template);
        let before = tree(root);
        let output = run(root, &["build", "::brief::main", "--format", "html", "-n"]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("invalid_html_template"));
        assert_eq!(tree(root), before);
    }
}
#[test]
fn html_cannot_overwrite_source_and_invalid_private_title_does_not_render() {
    let dir = setup("bundle");
    let root = dir.path();
    write(root, "brief/outputs/main.md", CONTENT);
    let before = tree(root);
    assert!(
        !run(root, &["build", "::brief::main", "--format", "html", "--out", "brief/outputs/main.md"]).status.success()
    );
    assert_eq!(tree(root), before);
    write(root, "brief/outputs/main.md", "---\nmind-forge-visibility: private\n---\nPRIVATE_TITLE");
    assert!(!run(root, &["build", "::brief::main", "--format", "html"]).status.success());
    assert!(!root.join("build/brief/main.html").exists());
}
