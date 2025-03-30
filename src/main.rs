use api_simulator::config;
use api_simulator::core::ApiSimulator;
use clap::{Command, Arg};
use log::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Debug)
        .init();

    // Parse command line arguments
    let matches = Command::new("translucent")
        .version("0.1.0")
        .about("Intelligent API simulator for testing")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("Sets the port to listen on")
                .value_parser(clap::value_parser!(u16)),
        )
        .arg(
            Arg::new("threads")
                .short('t')
                .long("threads")
                .value_name("NUM")
                .help("Number of worker threads")
                .value_parser(clap::value_parser!(usize)),
        )
        .get_matches();

    // Load configuration
    let config = config::load_config(matches)?;

    // Initialize the core simulator
    let simulator = ApiSimulator::new(config).await?;

    // Start the server
    simulator.run().await?;

    info!("Server started on");

    Ok(())
}
