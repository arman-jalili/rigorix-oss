//! Emit the R0 receipt schema (#1590): `cargo run -p rigorix-engine --example receipt_schema`.
fn main() -> Result<(), String> {
    print!("{}", rigorix_engine::receipt::receipt_schema_text()?);
    Ok(())
}
