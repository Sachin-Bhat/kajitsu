use std::sync::OnceLock;

use amane::Service;

use super::super::button::{self, Style};
use super::super::page::Page;
use super::super::{Confirm, Shown, row};

// asked once, a commit made while running shows after a restart
static COMMIT: OnceLock<String> = OnceLock::new();

pub fn build(page: &mut Page) {
    let commit = COMMIT.get_or_init(|| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(crate::paths::config_dir())
            .args(["rev-parse", "--short", "HEAD"])
            .output();
        let commit = output
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_else(|| "unknown".into());

        String::from(commit.trim())
    });

    let detail = format!("Kajitsu · hard fork of Suzuha · Git commit {commit}");

    action(
        page,
        "Config checkout",
        &detail,
        "Open folder",
        Style::Plain,
        || {
            let _ = std::process::Command::new("xdg-open")
                .arg(crate::paths::config_dir())
                .spawn();
        },
    );

    action(
        page,
        "Reset settings",
        "Restore every setting to its built-in default",
        "Reset",
        Style::Danger,
        || Shown::write().confirm = Some(Confirm::Reset),
    );

    page.end_group();
}

fn action(
    page: &mut Page,
    title: &str,
    detail: &str,
    label: &str,
    style: Style,
    on_click: impl Fn() + 'static,
) {
    let control = button::view(page.theme, label, style, true, on_click);

    let row = row::view(
        page.theme,
        page.width,
        row::HEIGHT,
        title,
        detail,
        control,
        button::width(label),
    );

    page.row(row::HEIGHT, row);
}
