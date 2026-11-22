//! SigmaOS component binary: iloveapi

use sigmaos_iloveapi::Component;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Component::new().operate()?;
    println!("sigmaos-iloveapi: ok");
    Ok(())
}
