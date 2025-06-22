use crate::models::CliApp;
use tracing::error;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
impl CliApp {
    pub async fn run_phase3(&self) -> Result<()> {
        println!("\n📤 Starting Phase 3: Exporting results to JSON files...");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let mut total_exported = 0;
        let mut export_failures = 0;

        for (i, source) in self.sources.iter().enumerate() {
            println!(
                "[{}/{}] Exporting: {}",
                i + 1,
                self.sources.len(),
                source.name()
            );

            match self.scraper.export_source_data(source.as_ref()).await {
                Ok(data) => {
                    let filename = format!(
                        "{}/{}.json",
                        self.config.output.directory,
                        source.output_filename()
                    );

                    match self.scraper.save_to_json(&data, &filename).await {
                        Ok(_) => {
                            total_exported += data.total_urls;
                            println!(
                                "✓ {} - Exported {} projects to {}",
                                source.name(),
                                data.total_urls,
                                filename
                            );
                        }
                        Err(e) => {
                            export_failures += 1;
                            error!("✗ Failed to save {}: {}", filename, e);
                        }
                    }
                }
                Err(e) => {
                    export_failures += 1;
                    error!("✗ {} - Export failed: {}", source.name(), e);
                }
            }
        }

        println!("\n🎉 Phase 3 Complete!");
        println!("Total projects exported: {}", total_exported);
        println!("Export failures: {}", export_failures);
        println!("Output directory: {}", self.config.output.directory);

        Ok(())
    }
}
