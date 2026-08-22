use crate::journal::{PerformanceAnalytics, TradeJournal};
use anyhow::Result;

pub fn run(export_md: Option<String>, initial_capital: f64) -> Result<()> {
    let journal = TradeJournal::new(None);
    let entries = journal.read_all()?;
    let summary = PerformanceAnalytics::compute_from_entries(&entries, initial_capital);

    PerformanceAnalytics::render_console_summary(&summary);

    if let Some(md_path) = export_md {
        let md_content = PerformanceAnalytics::render_markdown_report(&summary);
        let target = std::path::PathBuf::from(md_path);
        if let Some(p) = target.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        std::fs::write(&target, md_content)?;
        println!(
            "📄 Exported markdown review report to: {}",
            target.display()
        );
    }
    Ok(())
}
