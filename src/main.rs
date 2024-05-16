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
    path: PathBuf,
    #[clap(flatten)]
    geng: geng::CliArgs,
}

fn main() {
    let cli_args: CliArgs = cli::parse();
    Geng::run("thick sprite", move |geng| async move {
        let sprite: sprite_shape::ThickSprite<viewer::Vertex> = geng
            .asset_manager()
            .load_with(&cli_args.path, &{
                let mut options = geng_sprite_shape::Options::default();
                if let Some(cell_size) = cli_args.cell_size {
                    options.cell_size = cell_size;
                }
                if let Some(iso) = cli_args.iso {
                    options.iso = iso;
                }
                options
            })
            .await
            .unwrap();

        viewer::run(&geng, sprite).await;
    });
}
