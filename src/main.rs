use std::path::PathBuf;

use geng::prelude::*;
use geng_sprite_shape as sprite_shape;

mod viewer;

#[derive(clap::Parser)]
struct CliArgs {
    #[clap(long)]
    cell_size: Option<usize>,
    #[clap(long)]
    iso: Option<f32>,
    #[clap(long)]
    thickness: Option<f32>,
    #[clap(long)]
    back_face: Option<bool>,
    #[clap(long)]
    front_face: Option<bool>,
    path: PathBuf,
    #[clap(flatten)]
    geng: geng::CliArgs,
}

fn main() {
    let cli_args: CliArgs = cli::parse();
    Geng::run("thick sprite", move |geng| async move {
        let mut options = geng_sprite_shape::Options::default();
        macro_rules! options {
                    ($($op:ident,)*) => {
                        $(if let Some($op) = cli_args.$op {
                            options.$op = $op;
                        })*
                    }
                }
        options! {
            cell_size,
            iso,
            thickness,
            back_face,
            front_face,
        };
        viewer::Viewer::new(&geng, &cli_args.path, options)
            .await
            .run()
            .await;
    });
}
