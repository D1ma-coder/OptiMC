use anyhow::Result;
use clap::{Parser, Subcommand};
use optimc::{assets, net, render, world::lod};
use std::{path::PathBuf, time::Instant};
#[derive(Parser)]
#[command(
    name = "optimc",
    version,
    about = "Independent Rust/Vulkan client targeting Minecraft Java 26.2 (development build)"
)]
struct Cli {
    #[arg(long, global = true, default_value = ".cache")]
    cache: PathBuf,
    #[arg(long, global = true, value_enum)]
    quality: Option<optimc::settings::Quality>,
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    /// Build/update a macOS app bundle with the original official icon; no GPU.
    PackageApp {
        output: PathBuf,
        #[arg(long)]
        executable: Option<PathBuf>,
    },
    /// Open the Minecraft-style title menu.
    Menu {
        #[arg(long)]
        screenshot: Option<PathBuf>,
        #[arg(long)]
        graphics: bool,
    },
    /// Launch the Vulkan terrain laboratory (demo terrain, no server world yet).
    Demo {
        #[arg(long, default_value_t = 1024)]
        radius: i32,
        /// Save the first complete Vulkan terrain frame as PNG and exit.
        #[arg(long)]
        screenshot: Option<PathBuf>,
        #[arg(long)]
        closeup: bool,
        /// Exit after this many complete terrain frames (GPU/pacing verification).
        #[arg(long,value_parser=clap::value_parser!(u32).range(2..))]
        frames: Option<u32>,
        /// Spawn a walking player instead of the terrain laboratory camera.
        #[arg(long)]
        player: bool,
        /// Open the inventory immediately (implies --player).
        #[arg(long)]
        inventory: bool,
        #[arg(long, value_enum, default_value = "first")]
        perspective: optimc::player::Perspective,
    },
    /// Preview an official blockstate or model through Vulkan.
    Model {
        #[arg(default_value = "stone")]
        id: String,
        #[arg(long, default_value = "")]
        properties: String,
        /// Treat id as a model path rather than a blockstate.
        #[arg(long)]
        direct: bool,
        #[arg(long)]
        screenshot: Option<PathBuf>,
    },
    /// Download official resources or import a local Minecraft 26.2 client archive.
    Assets {
        #[command(subcommand)]
        action: AssetCommand,
    },
    /// Query Minecraft 26.2 server status and round-trip latency.
    Status { server: String },
    /// Exercise offline-mode login/configuration/play (diagnostic, not graphical gameplay).
    Connect {
        server: String,
        #[arg(long, default_value = "OptiPlayer")]
        username: String,
        #[arg(long, default_value_t = 30)]
        seconds: u64,
        /// Run normal idle player physics at 20 Hz without a window/GPU (diagnostic only).
        #[arg(long)]
        simulate_player: bool,
        /// Write decoded block/mesh counts without opening a window or using the GPU.
        #[arg(long)]
        mesh_report: Option<PathBuf>,
        /// Render a received world snapshot to PNG after disconnecting (visual QA).
        #[arg(long)]
        screenshot: Option<PathBuf>,
    },
    /// Open a live connection to an external offline-mode Java 26.2 server.
    Play {
        server: String,
        #[arg(long, default_value = "OptiPlayer")]
        username: String,
        #[arg(long)]
        screenshot: Option<PathBuf>,
        #[arg(long,value_parser=clap::value_parser!(u32).range(2..))]
        frames: Option<u32>,
    },
    /// List Vulkan adapters without opening a window.
    Gpu,
    /// Measure CPU terrain generation and LOD meshing without a GPU.
    Bench {
        #[arg(long, default_value_t = 512)]
        radius: i32,
    },
}
#[derive(Subcommand)]
enum AssetCommand {
    Sync,
    /// Resolve and bake every downloaded official blockstate/model.
    AuditModels,
    Import {
        jar: PathBuf,
    },
}
fn main() -> Result<()> {
    let mut cli = Cli::parse();
    if cli.cache == std::path::Path::new(".cache") {
        if let Some(cache) = std::env::var_os("OPTIMC_CACHE") {
            cli.cache = cache.into();
        }
    }
    if cli.cache == std::path::Path::new(".cache") && !cli.cache.join("26.2").exists() {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(cache) = exe
                .ancestors()
                .map(|p| p.join(".cache"))
                .find(|p| p.join("26.2").exists())
            {
                cli.cache = cache;
            }
        }
    }
    #[cfg(target_os = "macos")]
    if matches!(
        &cli.command,
        None | Some(Command::Menu { .. })
            | Some(Command::Demo { .. })
            | Some(Command::Model { .. })
            | Some(Command::Gpu)
            | Some(Command::Play { .. })
            | Some(Command::Connect {
                screenshot: Some(_),
                ..
            })
    ) {
        macos_vulkan_runtime(&cli.cache)?;
    }
    match cli.command.unwrap_or(Command::Menu {
        screenshot: None,
        graphics: false,
    }) {
        Command::Menu {
            screenshot,
            graphics,
        } => render::run(
            cli.cache,
            1024,
            screenshot,
            if graphics {
                render::ui::Screen::Graphics
            } else {
                render::ui::Screen::Main
            },
            cli.quality,
            render::Launch::default(),
        ),
        Command::Demo {
            radius,
            screenshot,
            closeup,
            frames,
            player,
            inventory,
            perspective,
        } => render::run(
            cli.cache,
            radius,
            screenshot,
            render::ui::Screen::Playing,
            cli.quality,
            render::Launch {
                closeup,
                frames,
                player: player || inventory,
                inventory,
                perspective,
                preview: None,
                remote_preview: None,
                server: None,
            },
        ),
        Command::Model {
            id,
            properties,
            direct,
            screenshot,
        } => {
            let mut values = assets::models::Properties::new();
            if !properties.is_empty() {
                for pair in properties.split(',') {
                    let (k, v) = pair
                        .split_once('=')
                        .ok_or_else(|| anyhow::anyhow!("Expected name=value properties"))?;
                    values.insert(k.into(), v.into());
                }
            }
            render::run(
                cli.cache,
                256,
                screenshot,
                render::ui::Screen::Playing,
                cli.quality,
                render::Launch {
                    preview: Some(render::Preview {
                        id,
                        properties: values,
                        direct,
                    }),
                    ..Default::default()
                },
            )
        }
        Command::Assets { action } => match action {
            AssetCommand::Sync => assets::sync(&cli.cache),
            AssetCommand::AuditModels => {
                assets::ensure_gui(&cli.cache)?;
                let report = assets::models::Catalog::new(&cli.cache).audit()?;
                println!("{}", serde_json::to_string_pretty(&report)?);
                Ok(())
            }
            AssetCommand::Import { jar } => assets::import_jar(&jar, &cli.cache),
        },
        Command::Status { server } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&net::status(&net::Address::parse(&server)?)?)?
            );
            Ok(())
        }
        Command::Connect {
            server,
            username,
            seconds,
            simulate_player,
            mesh_report,
            screenshot,
        } => {
            let address = net::Address::parse(&server)?;
            let report = if simulate_player {
                net::headless::session_logged(&address, &username, seconds, &cli.cache)?
            } else {
                net::session_state_logged(&address, &username, seconds, &cli.cache)?
            };
            if let Some(path) = mesh_report {
                let mut mesher = optimc::world::remote_mesh::RemoteMesher::new(&cli.cache);
                let (mut meshes, _) = mesher.update(&report.world)?;
                meshes.extend(mesher.frame_meshes(&report)?);
                meshes.extend(mesher.entity_meshes(&report)?);
                let columns: Vec<_> = report
                    .world
                    .chunks
                    .iter()
                    .map(|(&(x, z), column)| {
                        let nonzero_states: usize = column
                            .sections
                            .iter()
                            .map(|s| {
                                (0..4096)
                                    .filter(|&i| s.blocks.get(i).is_some_and(|v| v != 0))
                                    .count()
                            })
                            .sum();
                        serde_json::json!({"x":x,"z":z,"nonzero_states":nonzero_states})
                    })
                    .collect();
                let bounds: Vec<_> = meshes.iter().map(|m| serde_json::json!({"min":m.min.to_array(),"max":m.max.to_array(),"triangles":m.indices.len()/3})).collect();
                std::fs::write(
                    path,
                    serde_json::to_vec_pretty(
                        &serde_json::json!({"columns":columns,"meshes":bounds,"textures":mesher.texture_versions().len(),"entities":report.entities.entities.len(),"unhandled":report.unhandled}),
                    )?,
                )?;
            }
            if let Some(path) = screenshot {
                render::run(
                    cli.cache,
                    4096,
                    Some(path),
                    render::ui::Screen::Playing,
                    cli.quality,
                    render::Launch {
                        remote_preview: Some(report.world),
                        ..Default::default()
                    },
                )
            } else {
                Ok(())
            }
        }
        Command::Play {
            server,
            username,
            screenshot,
            frames,
        } => render::run(
            cli.cache,
            4096,
            screenshot,
            render::ui::Screen::Connecting,
            cli.quality,
            render::Launch {
                server: Some((net::Address::parse(&server)?, username)),
                frames,
                ..Default::default()
            },
        ),
        Command::PackageApp { output, executable } => {
            let executable = executable.unwrap_or(std::env::current_exe()?);
            optimc::packaging::macos_app(&executable, &cli.cache, &output)?;
            println!("Packaged {}", output.display());
            Ok(())
        }
        Command::Gpu => render::probe(),
        Command::Bench { radius } => {
            let now = Instant::now();
            let eye = glam::Vec3::new(0., 85., 100.);
            let tiles = lod::Lod::new(radius).select(eye);
            let meshes = lod::build(&tiles, eye);
            let bytes = meshes
                .iter()
                .map(|m| {
                    m.vertices.len() * std::mem::size_of::<optimc::world::Vertex>()
                        + m.indices.len() * 4
                })
                .sum::<usize>();
            println!(
                "{} tiles · {} triangles · {:.2} MiB · {:.2} ms",
                meshes.len(),
                meshes.iter().map(|m| m.indices.len() / 3).sum::<usize>(),
                bytes as f64 / 1048576.,
                now.elapsed().as_secs_f64() * 1000.
            );
            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_vulkan_runtime(cache: &std::path::Path) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let lib = cache.join("runtime/lib");
    if lib.join("libvulkan.dylib").exists()
        && std::env::var_os("DYLD_FALLBACK_LIBRARY_PATH").is_none()
    {
        // dyld reads its search path at process startup, so a one-time re-exec is
        // needed for a project-local MoltenVK install. A system SDK also works.
        let error = std::process::Command::new(std::env::current_exe()?)
            .args(std::env::args_os().skip(1))
            .env("DYLD_FALLBACK_LIBRARY_PATH", lib.canonicalize()?)
            .env("MVK_CONFIG_LOG_LEVEL", "1")
            .exec();
        return Err(error.into());
    }
    Ok(())
}
