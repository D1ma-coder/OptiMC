//! CPU-only diagnosis: retains the last world even if a server rejects the session.
use anyhow::Result;
use optimc::{
    net::{
        live::{Connection, Status},
        Address,
    },
    world::remote_mesh::RemoteMesher,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    anyhow::ensure!(args.len() == 3, "inspect_remote SERVER USERNAME");
    let connection = Connection::connect_logged(
        Address::parse(&args[1])?,
        args[2].clone(),
        Path::new(".cache"),
    )?;
    let until = Instant::now() + Duration::from_secs(20);
    let mut last = None;
    while Instant::now() < until {
        if let Some(snapshot) = connection.take_snapshot() {
            last = Some(snapshot);
        }
        if matches!(connection.status(), Status::Closed(_) | Status::Rejected(_)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    connection.cancel();
    println!("Status: {:?}", connection.status());
    if let Some(s) = last {
        println!(
            "Position {:?}, dimension {:?}, entities {}",
            s.position,
            s.report.world.dimension,
            s.report.entities.entities.len()
        );
        for ((x, z), c) in &s.report.world.chunks {
            let mut states = std::collections::BTreeMap::new();
            for section in &c.sections {
                for i in 0..4096 {
                    *states
                        .entry(section.blocks.get(i).unwrap())
                        .or_insert(0usize) += 1;
                }
            }
            println!("Chunk {x},{z}: {states:?}");
        }
        if s.report.world.dimension.is_some() {
            let mut mesher = RemoteMesher::new(Path::new(".cache"));
            let (meshes, stats) = mesher.update(&s.report.world)?;
            println!("Meshes {}, statistics {stats:?}", meshes.len());
            for mesh in meshes {
                println!(
                    "Bounds {:?} {:?}, triangles {}",
                    mesh.min,
                    mesh.max,
                    mesh.indices.len() / 3
                );
            }
        }
    }
    Ok(())
}
