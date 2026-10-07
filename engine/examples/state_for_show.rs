use rpi_vj_native::{control::State, show::Show};
fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("show path");
    let show = Show::load(std::path::Path::new(&path))?;
    println!("{}", serde_json::to_string(&State::from_show(&show))?);
    Ok(())
}
