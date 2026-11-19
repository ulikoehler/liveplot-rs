//! Shared-VkDevice bring-up: wrap VolcanoPlot's process-wide Vulkan
//! objects into wgpu so eframe can render on the *same* VkDevice the
//! C++ renderer uses — the first step of the zero-copy frame path
//! (docs/volcano-shared-device-plan.md, Phase 0).
//!
//! Bring-up order matters: [`init`] must run before the first
//! `VpFigure` is created so the shared context is built in
//! extended-sharing mode (all supported extensions/features on). The
//! caller is responsible for checking [`SharedInitError`] and falling
//! back to the CPU readback path.
//!
//! Safety invariants (all `unsafe` lives in this file):
//! - VolcanoPlot owns the VkInstance/VkDevice — every `from_raw`
//!   wrapper gets a no-op drop callback so wgpu never destroys them.
//! - `enabled_extensions`/`features` reported to `device_from_raw`
//!   exactly match what the C++ device was created with (all
//!   supported), so wgpu's required-extension check passes.
//! - The shared queue is `graphics_family`/index 0 — the same VkQueue
//!   VolcanoPlot's workers submit on, under `globalQueueMutex`. Rust
//!   raw submissions on it happen only on the UI thread and must be
//!   serialized against wgpu's own submits the same way (same thread
//!   → program order). Worker-side C++ submits on the same queue are
//!   spec-UB in shm mode — the record/submit split (Phase 1+) makes
//!   the UI thread the sole submitter.

use std::ffi::CStr;

use ash::vk::Handle as _;
use eframe::wgpu;

use super::VpSharedGpuInfo;

/// Everything eframe needs to run on VolcanoPlot's device.
pub struct SharedGraphics {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// VkQueue family+index wgpu grabbed — the same handle VolcanoPlot
    /// submits on (this GPU exposes a single queue per family).
    pub queue_family: u32,
    /// egui-wgpu renderer state — attached after eframe creation
    /// (`attach_render_state`) so surfaces can register native
    /// textures without threading RenderState through every call.
    render_state: std::sync::Mutex<Option<eframe::egui_wgpu::RenderState>>,
}

static SHARED: std::sync::OnceLock<SharedGraphics> = std::sync::OnceLock::new();

/// The process-wide shared device — `Some` only after a successful
/// [`init`].
pub fn shared() -> Option<&'static SharedGraphics> {
    SHARED.get()
}

/// Give surfaces access to egui's renderer (native-texture
/// registration needs `RenderState.renderer`). Called once from the
/// app-creation context where `wgpu_render_state` is available.
pub fn attach_render_state(rs: eframe::egui_wgpu::RenderState) {
    if let Some(s) = SHARED.get() {
        *s.render_state.lock().unwrap() = Some(rs);
    }
}

impl SharedGraphics {
    /// Clone of the egui-wgpu `RenderState` (None until attached).
    pub fn render_state(&self) -> Option<eframe::egui_wgpu::RenderState> {
        self.render_state.lock().unwrap().clone()
    }

    /// Wrap a foreign `VkBuffer` (VolcanoPlot-owned staging buffer) as
    /// a `wgpu::Buffer` usable as a `copy_buffer_to_texture` source.
    /// Externally owned: wgpu never destroys it; the caller guarantees
    /// the VkBuffer outlives the wrapper.
    ///
    /// # Safety
    /// `vk_buf` must be a valid buffer on this device, live for the
    /// wrapper's lifetime, and at least `size` bytes.
    pub unsafe fn wrap_buffer(
        &self,
        vk_buf: ash::vk::Buffer,
        size: u64,
    ) -> Result<wgpu::Buffer, String> {
        let hal_dev = self
            .device
            .as_hal::<wgpu_hal::api::Vulkan>()
            .ok_or("no vulkan hal device")?;
        let hal_buf = wgpu_hal::vulkan::Buffer::from_raw_externally_owned(vk_buf, Box::new(|| {}));
        let _ = hal_dev;
        Ok(self.device.create_buffer_from_hal::<wgpu_hal::api::Vulkan>(
            hal_buf,
            &wgpu::BufferDescriptor {
                label: Some("vp-shm-frame"),
                size,
                usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        ))
    }
}

#[derive(Debug)]
pub enum SharedInitError {
    /// `vp_shared_gpu_info` returned failure — no Vulkan.
    NoGpuContext,
    /// A figure was created before the probe, so the shared context
    /// exists without the extended extension/feature set.
    NotSharedCapable,
    /// wgpu-hal rejected the physical device or foreign objects.
    Hal(String),
}

impl std::fmt::Display for SharedInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoGpuContext => write!(f, "vp_shared_gpu_info failed (no Vulkan)"),
            Self::NotSharedCapable => write!(
                f,
                "shared GPU context is not extended — a VpFigure was \
                 created before shared init"
            ),
            Self::Hal(e) => write!(f, "wgpu-hal interop error: {e}"),
        }
    }
}

impl std::error::Error for SharedInitError {}

/// Collect the `const char**` array an FFI getter returns into a vec
/// of CStrs. The pointees live in the C++ context's static storage —
/// leaked by design — so `'static` is accurate.
unsafe fn ext_list(f: unsafe extern "C" fn(*mut *const *const i8) -> i32) -> Vec<&'static CStr> {
    let mut arr: *const *const i8 = std::ptr::null();
    let n = f(&mut arr);
    if n <= 0 || arr.is_null() {
        return Vec::new();
    }
    (0..n as isize)
        .map(|i| CStr::from_ptr(*arr.offset(i)))
        .collect()
}

/// Bring up wgpu on VolcanoPlot's shared VkDevice.
///
/// Must be called before the first `VolcanoFigure`/`vp_figure_create`
/// or the context won't be extended and this returns
/// `NotSharedCapable`.
pub fn init() -> Result<SharedGraphicsRef, SharedInitError> {
    unsafe { init_inner() }
}

unsafe fn init_inner() -> Result<SharedGraphicsRef, SharedInitError> {
    // 1. Ask C++ for the shared context handles (creates it extended
    //    if this is the first touch).
    let mut info = VpSharedGpuInfo::default();
    if super::vp_shared_gpu_info(&mut info) != 1 {
        return Err(SharedInitError::NoGpuContext);
    }
    if info.shared_capable == 0 {
        return Err(SharedInitError::NotSharedCapable);
    }
    let inst_exts = ext_list(super::vp_shared_instance_exts);
    let dev_exts = ext_list(super::vp_shared_device_exts);

    // 2. Wrap the foreign VkInstance.
    let entry = ash::Entry::load().map_err(|e| SharedInitError::Hal(format!("ash entry: {e}")))?;
    let ash_inst = ash::Instance::load(
        entry.static_fn(),
        ash::vk::Instance::from_raw(info.vk_instance),
    );
    let hal_inst = wgpu_hal::vulkan::Instance::from_raw(
        entry,
        ash_inst.clone(),
        info.api_version,
        0,    // android_sdk_version
        None, // no debug-messenger — VP may own its own
        inst_exts,
        wgpu::InstanceFlags::default(),
        wgpu::MemoryBudgetThresholds::default(),
        false,                 // has_nv_optimus
        Some(Box::new(|| {})), // no-op drop cb: VP owns the VkInstance
    )
    .map_err(|e| SharedInitError::Hal(format!("instance wrap: {e:?}")))?;
    let instance = wgpu::Instance::from_hal::<wgpu_hal::api::Vulkan>(hal_inst);

    // 3. Expose VP's physical device as a wgpu adapter.
    let exposed = {
        let hal_inst_guard = instance.as_hal::<wgpu_hal::api::Vulkan>();
        let hal_inst_ref =
            hal_inst_guard.ok_or_else(|| SharedInitError::Hal("no vulkan hal instance".into()))?;
        hal_inst_ref.expose_adapter(ash::vk::PhysicalDevice::from_raw(info.vk_physical_device))
    }
    .ok_or_else(|| SharedInitError::Hal("expose_adapter rejected the physical device".into()))?;
    let hal_adapter = &exposed.adapter;

    // 4. Contract check: every extension wgpu needs must already be
    //    enabled on the foreign device (we enabled all supported —
    //    this should trivially hold).
    let features = wgpu::Features::default();
    let required = hal_adapter.required_device_extensions(features);
    let missing: Vec<_> = required
        .iter()
        .filter(|r| !dev_exts.iter().any(|e| e == r))
        .collect();
    if !missing.is_empty() {
        return Err(SharedInitError::Hal(format!(
            "device missing wgpu-required extensions: {missing:?}"
        )));
    }

    // 5. Wrap the foreign VkDevice.
    let ash_dev = ash::Device::load(
        ash_inst.fp_v1_0(),
        ash::vk::Device::from_raw(info.vk_device),
    );
    let limits = wgpu::Limits::default();
    let open = hal_adapter
        .device_from_raw(
            ash_dev,
            Some(Box::new(|| {})), // no-op: VP owns vkDestroyDevice
            &dev_exts,
            features,
            &limits,
            &wgpu::MemoryHints::Performance,
            info.graphics_family,
            0, // queue_index — single queue shared with VP
        )
        .map_err(|e| SharedInitError::Hal(format!("device_from_raw: {e:?}")))?;
    let adapter = instance.create_adapter_from_hal::<wgpu_hal::api::Vulkan>(exposed);
    let (device, queue) = adapter
        .create_device_from_hal::<wgpu_hal::api::Vulkan>(open, &wgpu::DeviceDescriptor::default())
        .map_err(|e| SharedInitError::Hal(format!("create_device_from_hal: {e}")))?;

    eprintln!(
        "[vp-shm] shared VkDevice bring-up ok: adapter={:?} family={}",
        adapter.get_info().name,
        info.graphics_family
    );
    let sg = SharedGraphics {
        instance,
        adapter,
        device,
        queue,
        queue_family: info.graphics_family,
        render_state: std::sync::Mutex::new(None),
    };
    // Stash for surface access — keep the returned handle usable too.
    let _ = SHARED.set(sg);
    Ok(SharedGraphicsRef(SHARED.get().unwrap()))
}

/// Shared handle into the global [`SharedGraphics`].
#[derive(Clone, Copy)]
pub struct SharedGraphicsRef(&'static SharedGraphics);

impl std::ops::Deref for SharedGraphicsRef {
    type Target = SharedGraphics;
    fn deref(&self) -> &SharedGraphics {
        self.0
    }
}
