//! Berserker workload generator.
//!
//! The implementation is covering two part:
//! * workload independent logic
//! * workload specific details
//!
//! Those have to be isolated as much as possible, and working together via
//! configuration data structures and worker interface.
//!
//! The execution contains following steps:
//! * Consume provided configuration
//! * For each available CPU core spawn specified number of worker processes
//! * Invoke a workload-specific logic via run_payload
//! * Wait for all the workers to finish

#[macro_use]
extern crate log;
extern crate core_affinity;

use config::Config;

use berserker::WorkloadConfig;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let default_config = String::from("workload.toml");
    let config_path = &args.get(1).unwrap_or(&default_config);

    let config = Config::builder()
        // Add in `./Settings.toml`
        .add_source(
            config::File::with_name("/etc/berserker/workload.toml")
                .required(false),
        )
        .add_source(config::File::with_name(config_path).required(false))
        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `BERSERKER__WORKLOAD__ARRIVAL_RATE=1` would set the `arrival_rate` key
        .add_source(
            config::Environment::with_prefix("BERSERKER")
                .try_parsing(true)
                .separator("__"),
        )
        .build()
        .unwrap()
        .try_deserialize::<WorkloadConfig>()
        .unwrap();

    env_logger::init();

    info!("Config: {:?}", config);

    berserker::run(config);
}
