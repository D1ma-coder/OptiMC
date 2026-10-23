//! Optional non-blocking GPU timestamps plus CPU submission/presentation timings.
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
pub struct Profile {
    query: Option<wgpu::QuerySet>,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    pending: Option<mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
    gpu_ms: f64,
    samples: u32,
    totals: [f64; 4],
    period: f64,
}
impl Profile {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let query = device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| {
                device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("Scene GPU timing"),
                    ty: wgpu::QueryType::Timestamp,
                    count: 2,
                })
            });
        Self {
            query,
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 256,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            read: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 16,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            pending: None,
            gpu_ms: 0.,
            samples: 0,
            totals: [0.; 4],
            period: queue.get_timestamp_period() as f64,
        }
    }
    pub fn poll(&mut self, device: &wgpu::Device) {
        device.poll(wgpu::Maintain::Poll);
        if let Some(done) = &self.pending {
            if let Ok(result) = done.try_recv() {
                if result.is_ok() {
                    let data = self.read.slice(..).get_mapped_range();
                    let ticks: &[u64] = bytemuck::cast_slice(&data);
                    if ticks[0] > 0 && ticks[1] > ticks[0] {
                        self.gpu_ms = (ticks[1] - ticks[0]) as f64 * self.period / 1e6;
                    }
                    drop(data);
                    self.read.unmap();
                }
                self.pending = None;
            }
        }
    }
    pub fn timestamps(&self) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        if self.pending.is_some() {
            return None;
        }
        self.query
            .as_ref()
            .map(|query_set| wgpu::RenderPassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            })
    }
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) -> bool {
        if self.pending.is_some() {
            return false;
        }
        if let Some(query) = &self.query {
            encoder.resolve_query_set(query, 0..2, &self.resolve, 0);
            encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, 16);
            return true;
        }
        false
    }
    pub fn submitted(&mut self, sampled: bool) {
        if !sampled {
            return;
        }
        let (send, receive) = mpsc::channel();
        self.read
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = send.send(result);
            });
        self.pending = Some(receive);
    }
    pub fn frame(
        &mut self,
        acquire: Duration,
        encode: Duration,
        submit: Duration,
        present: Duration,
    ) {
        for (total, time) in self
            .totals
            .iter_mut()
            .zip([acquire, encode, submit, present])
        {
            *total += time.as_secs_f64() * 1000.;
        }
        self.samples += 1;
        if self.samples == 240 {
            println!("Frame profile (ms): acquire {:.3}, encode {:.3}, submit {:.3}, present {:.3}; latest scene GPU {:.3}",self.totals[0]/240.,self.totals[1]/240.,self.totals[2]/240.,self.totals[3]/240.,self.gpu_ms);
            self.samples = 0;
            self.totals = [0.; 4];
        }
    }
}
pub fn now() -> Instant {
    Instant::now()
}
