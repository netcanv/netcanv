use crate::gpu::Gpu;

pub struct BatchStorageConfig {
   pub name: &'static str,
   pub buffer_size: wgpu::BufferAddress,
   pub bind_group_layout: wgpu::BindGroupLayout,
}

pub struct BatchStorage {
   config: BatchStorageConfig,
   buffers: Vec<wgpu::Buffer>,
   bind_groups: Vec<wgpu::BindGroup>,
   current_batch: usize,
}

impl BatchStorage {
   pub fn new(config: BatchStorageConfig) -> Self {
      Self {
         config,
         buffers: vec![],
         bind_groups: vec![],
         current_batch: 0,
      }
   }

   pub fn next_batch(&mut self, gpu: &Gpu) -> (&wgpu::Buffer, &wgpu::BindGroup) {
      if self.buffers.get(self.current_batch).is_none() {
         let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!(
               "{}: Data Buffer #{}",
               self.config.name,
               self.buffers.len()
            )),
            size: self.config.buffer_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
         });
         let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!(
               "{}: Bind Group #{}",
               self.config.name,
               self.bind_groups.len()
            )),
            layout: &self.config.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
               binding: 0,
               resource: buffer.as_entire_binding(),
            }],
         });
         self.buffers.push(buffer);
         self.bind_groups.push(bind_group);
      }
      let batch = (
         &self.buffers[self.current_batch],
         &self.bind_groups[self.current_batch],
      );
      self.current_batch += 1;
      batch
   }

   pub fn rewind(&mut self) {
      self.current_batch = 0;
   }
}
