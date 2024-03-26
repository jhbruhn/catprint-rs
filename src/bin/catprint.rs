use btleplug::platform::Manager;
use catprint::*;
use std::error::Error;

use clap::{crate_authors, crate_version, AppSettings, ArgEnum, Clap};

#[derive(Clap)]
#[clap(version = crate_version!(), author = crate_authors!())]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    /// Set the device name of your printer.
    #[clap(short, long, default_value = "GB02")]
    device_name: String,
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    /// Move the paper without printing
    Feed(Feed),

    /// Print an image
    Print(Print),
}

#[derive(Clap)]
struct Feed {
    /// Print debug info
    #[clap(short, long)]
    backward: bool,
    /// amount to feed the paper
    length: u8,
}

#[derive(Clap)]
struct Print {
    /// The path to the image you want to print out
    input_image: String,

    /// The ditherer supposed to be used. none is good for text and vector graphics
    #[clap(arg_enum, short, long, default_value = "k-mean")]
    ditherer: Ditherers,

    /// Rotate picture by 90 degrees
    #[clap(short, long)]
    rotate: bool,

    /// Don't use compression
    #[clap(long)]
    no_compress: bool,
}

#[derive(ArgEnum)]
enum Ditherers {
    None,
    KMean,
    Atkinson,
    Burkes,
    FloydSteinberg,
    JarvisJudiceNinke,
    Sierra3,
    Stucki,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opts: Opts = Opts::parse();

    let mut manager = Manager::new().await?;

    let device = device::Device::find(&mut manager, &opts.device_name).await?;

    match opts.subcmd {
        SubCommand::Feed(feed) => main_feed(device, feed).await,
        SubCommand::Print(print) => main_print(device, print).await,
    }
}

async fn main_feed(mut device: device::Device, feed: Feed) -> Result<(), Box<dyn Error>> {
    let feed_direction = if feed.backward {
        protocol::FeedDirection::Reverse
    } else {
        protocol::FeedDirection::Forward
    };

    device.queue_command(protocol::Command::Feed(feed_direction, feed.length));

    device.flush().await
}

async fn main_print(mut device: device::Device, print: Print) -> Result<(), Box<dyn Error>> {
    device.queue_command(protocol::Command::Feed(
        protocol::FeedDirection::Forward,
        10,
    ));

    let image =
        image::Image::load(&std::path::PathBuf::from(print.input_image), print.rotate).unwrap();

    let image = match print.ditherer {
        Ditherers::None => image,
        Ditherers::KMean => image.kmean(),
        Ditherers::Atkinson => image.dither(&dither::ditherer::ATKINSON),
        Ditherers::Burkes => image.dither(&dither::ditherer::BURKES),
        Ditherers::FloydSteinberg => image.dither(&dither::ditherer::FLOYD_STEINBERG),
        Ditherers::JarvisJudiceNinke => image.dither(&dither::ditherer::JARVIS_JUDICE_NINKE),
        Ditherers::Sierra3 => image.dither(&dither::ditherer::SIERRA_3),
        Ditherers::Stucki => image.dither(&dither::ditherer::STUCKI),
    };

    let quality = protocol::Quality::Quality3;
    let energy = 12000;
    let mode = match print.ditherer {
        Ditherers::None | Ditherers::KMean => protocol::DrawingMode::Text,
        _ => protocol::DrawingMode::Image,
    };

    let use_compression = device.supports_compression() && !print.no_compress;
    let print = image.print(mode, quality, energy, use_compression);

    device.queue_commands(&print);
    device.queue_command(protocol::Command::Feed(
        protocol::FeedDirection::Forward,
        150,
    ));

    device.flush().await?;

    Ok(())
}
