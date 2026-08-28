use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let resources = PathBuf::from(args.next().ok_or("missing source resources directory")?);
    let output = PathBuf::from(args.next().ok_or("missing output directory")?);
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    gonnyu_resource_build::build(&resources, &output)?;
    Ok(())
}
