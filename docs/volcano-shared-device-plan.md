# VolcanoPlot ↔ wgpu Shared-Device Plan

Goal: eliminate the per-frame CPU round-trip between the VolcanoPlot
Vulkan renderer and egui/wgpu, cutting Volcano-mode CPU load toward
egui_plot parity while keeping the GPU-side trace pipeline unchanged.

Status: design plan. No code in this document is committed behavior.
Applies to: `liveplot-rs` branch `volcanoplot`,
`vendor/VolcanoPlot` branch `liveplot-ffi`, wgpu 30.0.1, egui-wgpu 0.37,
ash 0.38, Linux/RADV (primary target), Windows/macOS as follow-up.

---

## 0. Measured motivation

Same binary, same workload (8 traces × 20 kHz × 10 s window,
~1.6 M points/frame, 60 Hz UI), release build, Radeon iGPU/RADV, X11:

| Backend   | CPU (% of 1 core) | GPU busy | UI fps | RSS    |
|-----------|-------------------|----------|--------|--------|
| egui_plot | 17.3              | 34 %     | ~52    | 226 MB |
| Volcano   | 43.4              | 61 %     | ~52    | 335 MB |

Per-thread breakdown of the Volcano run:

- render worker thread: ~26 %  (≈4.4 ms CPU/frame)
- UI thread:            ~12.6 % (egui repaint + `write_texture` upload)

Where the worker's ~4.4 ms/frame goes:

- ~1.5–2 ms: 3.7 MB readback memcpy (staging → `ColorImage`)
- ~2–2.5 ms: Vulkan record/submit + `prepare` orchestration
- ~0.1 ms: trace uploads (already incremental ring appends)

Plus on the UI thread ~1 ms: `queue.write_texture` re-upload of the
same 3.7 MB into a wgpu texture.

**The entire GPU→CPU→GPU frame-buffer round-trip (~3 ms CPU/frame,
~18 % CPU, 7.4 MB/frame of PCIe traffic on a dGPU) exists only because
the two Vulkan devices cannot see each other's memory.** That is the
target of this plan. Everything else (trace ingest, envelope decimation,
tessellation) already happens GPU-side.

What this plan does NOT try to fix:

- GPU-side render cost (~8–10 ms wall on iGPU) — bounded by the
  transform/envelope/raster work, unchanged by interop.
- The `rawF64_` + GPU-buffer point copies (~4× data amplification at
  steady state) — a separate memory-footprint effort.
- CPU cost on non-Vulkan wgpu backends (GL/DX12/Metal) — those keep the
  existing readback path as fallback.

---

## 1. Current pipeline anatomy

```
producer thread ──push──► TraceStore (VecDeque<PlotPoint>)        [Rust]
                              │
UI thread                     │ get_points_ref + slide bookkeeping
   │                          ▼
   │                    FigJob{ traces:[tail slices], sig }  ──► mpsc
   │                                                              │
   │                                   worker thread              ▼
   │                    ┌──────────────────────────────────────────────┐
   │                    │ vp_trace_slide_data_f64  (ring append)       │
   │                    │ vp_render_rgba_view:                       │
   │                    │   renderer->prepare()   (uploads, autoscale) │
   │                    │   renderer->renderFrame()                  │
   │                    │     record cmd buf on VP VkDevice            │
   │                    │     vkQueueSubmit + fence wait (sleep-poll)  │
   │                    │   vkCmdCopyImageToBuffer → host staging      │
   │                    │   return view into mapped staging            │
   │                    │ ColorImage::from_rgba_unmultiplied (memcpy)  │
   │                    └──────────────────────────────────────────────┘
   │                                                              │
   ◄──────────────────── RenderReport{ColorImage, size} ────────────┘
   │
   ▼
texture_manager.set(TextureId::User, ColorImage)  → write_texture 3.7MB
egui Image draws the texture
```

The two device objects involved:

- `VpFigure` owns `volcano::core::Device` (own `VkInstance`, own
  `VkPhysicalDevice`, own `VkDevice`, own queues, own VMA allocator),
  created inside `vp_create_figure` — C++ side, Vulkan-Hpp.
- egui-wgpu owns `wgpu::Instance/Adapter/Device/Queue`, created by
  eframe's `WgpuConfiguration` — Rust side.

They are different `VkDevice` objects (and possibly different
`VkInstance`s) on the same `VkPhysicalDevice` — hence the mandatory
CPU bridge today.

---

## 2. Design space

### Option A — one shared VkDevice (RECOMMENDED)

VolcanoPlot creates its `VkDevice` as today, but with the extension and
feature set **unioned with what wgpu-hal requires**. Rust then wraps
that same device into a `wgpu::Device` via `wgpu-hal`'s `from_raw`
entry points and hands it to eframe via `WgpuSetupExisting`.

Consequences:

- The VolcanoPlot render-target `VkImage` is a plain image *on the same
  device*. Wrapping it as a `wgpu::Texture` is a bookkeeping operation
  (`texture_from_raw` + `create_texture_from_hal`) — no `VK_KHR_external_*`,
  no dma-buf, no DRM modifiers, no fd passing.
- Synchronization is ordinary same-device semaphores, or even pure
  queue ordering if VolcanoPlot submits on wgpu's raw queue.
- Lifetime: the C++ side must not destroy the VkDevice while wgpu
  objects live; enforce by construction order (VP first, eframe second)
  and a drop-order contract documented in FFI.

Risk profile: medium. The fragile part is feature/extension negotiation
(wgpu refuses adapters missing required extensions) and wgpu-hal's
assumption that `enabled_extensions`/`features` passed to
`device_from_raw` describe the foreign device truthfully.

### Option B — cross-device dma-buf import

Keep both devices. Export the VolcanoPlot render target as a dma-buf
(`VK_KHR_external_memory` + `VK_EXT_external_memory_dma_buf` +
`VK_EXT_image_drm_format_modifier`), import it on wgpu's raw
`ash::Device` via `vkCreateImage` + `VkImportMemoryFdInfoKHR`, wrap with
`texture_from_raw`.

Synchronization across devices needs `VK_KHR_external_semaphore_fd`
with `SYNC_FD` handle types: VP signals an exportable semaphore →
`vkGetSemaphoreFdKHR` → import into a wgpu-side semaphore →
wait-injection `vkQueueSubmit` on wgpu's raw queue before egui submits.

Risk profile: high. DRM modifier negotiation, per-frame sync_fd churn,
driver-version variance, and two device lifetimes to keep in step.
Chosen only as fallback if Option A fails, e.g. if eframe must own
device creation (macOS/Metal later, or a wgpu feature VP's device
cannot enable).

### Option C — shared staging buffer only (cheap Phase-1 win)

Render target stays private to VP. After `renderFrame`, VP does
`vkCmdCopyImageToBuffer` into a **VkBuffer** that lives on the shared
device (Option A) or is dma-buf-imported (Option B). wgpu then does a
GPU-side `copyBufferToTexture` via a tiny helper encoder → texture.

- Still fully zero-CPU-copy (the ~3 ms memcpy+upload vanishes).
- Adds one extra GPU-side copy of 3.7 MB — ~0.1 ms on iGPU, negligible.
- Sidesteps image-layout/modifier issues entirely: buffer import is
  far simpler than image import (no tiling, no layout transitions).
- With Option A (same device) the "import" is literally
  `create_buffer_from_hal` on a VkBuffer VP already owns.

Risk profile: low. Recommended as the **first milestone** of Option A
because it de-risks device sharing before touching image wrap/layout.

### Option D — port VolcanoPlot to wgpu-hal / egui paint callbacks

Rewrite the VP render pipeline on `wgpu-hal` so it executes inside
egui's render pass via `egui_wgpu::Callback`. Eliminates all interop
but is effectively a full renderer port — weeks of work, loses the
existing C++ investment. Not recommended; listed for completeness.

### Decision

```
Phase 1  (de-risk)  : Option A device sharing + Option C shared-buffer copy
Phase 2  (full win) : Option A direct image wrap (no GPU copy at all)
Fallback            : Option B (dma-buf) if device sharing is impossible
                      on a given platform; CPU readback path stays for
                      non-Vulkan backends
```

---

## 3. Target architecture (Option A, Phase 2 end state)

```
┌─────────────────────────────── one VkDevice ───────────────────────────┐
│                                                                        │
│  VolcanoPlot (C++)                    egui / wgpu (Rust)               │
│  ─────────────────                    ──────────────────               │
│  VkInstance  ──────────┐                                               │
│  VkPhysicalDevice ─────┼──► expose_adapter ──► wgpu::Adapter          │
│  VkDevice ─────────────┼──► device_from_raw ──► wgpu::Device          │
│  (extensions+features  │                    ──► wgpu::Queue           │
│   = VP ∪ wgpu-hal)     │                           │                   │
│  queues (own handles)  │        WgpuSetupExisting ─┘                   │
│  VMA on shared device  │                    eframe::run_native         │
│                        │                                               │
│  render target image[s]│──► texture_from_raw ──► wgpu::Texture        │
│   (double-buffered)    │        TextureId::User → egui::Image          │
│                        │                                               │
│  render sem[i] ────────┼──► wait-inject vkQueueSubmit on raw_queue    │
│  (same-device binary)  │    before egui's frame submission             │
│                        │                                               │
│  slot release ◄────────┼──── Queue::on_submitted_work_done            │
│                        │                                               │
└────────────────────────────────────────────────────────────────────────┘
```

Key properties:

- No CPU pixel copies anywhere in the steady-state path.
- Worker thread still does `prepare` + record + submit — its CPU drops
  to ~2–2.5 ms/frame (record+submit only; the memcpy vanishes).
- UI thread's `write_texture` upload vanishes → UI CPU drops to
  egui-repaint only (~5–8 %).
- Expected total: ~42 % → ~15–20 % — egui_plot parity territory.

---

## 4. Verified API surface (wgpu 30.0.1 / wgpu-hal 30.0.1 / egui-wgpu 0.37)

All paths below were confirmed in the vendored registry sources
(`~/.cargo/registry/src/...`). Signature paraphrase; check exact bounds
when writing code — these APIs shift between wgpu releases.

### 4.1 Instance

```rust
// wgpu-hal/src/vulkan/instance.rs:317
impl hal::vulkan::Instance {
    pub unsafe fn from_raw(
        entry: ash::Entry,
        instance: ash::Instance,
        // + version, extensions, flags, has_nv_optimus, drop_callback, ...
    ) -> Result<Self, ...>;

    // instance.rs:180
    pub fn raw_instance(&self) -> &ash::Instance;

    // adapter.rs:2219 — wrap ANY VkPhysicalDevice as a wgpu adapter
    pub unsafe fn expose_adapter(
        &self,
        phd: vk::PhysicalDevice,
    ) -> Option<ExposedAdapter<super::Api>>;
}

// wgpu/src/api/instance.rs:392
impl wgpu::Instance {
    pub unsafe fn create_adapter_from_hal<A: hal::Api>(
        &self, hal_adapter: A::Adapter,
    ) -> wgpu::Adapter;

    // api/instance.rs:335
    pub unsafe fn from_hal<A: hal::Api>(hal_instance: A::Instance) -> Self;
}
```

`expose_adapter` runs wgpu's full capability check
(`vkGetPhysicalDeviceFeatures2`, extension presence, driver quirks) —
it returns `None` for devices it can't drive, which is exactly the
guard we want.

### 4.2 Adapter → Device

```rust
// wgpu-hal/src/vulkan/adapter.rs:2516
impl hal::vulkan::Adapter {
    pub fn required_device_extensions(
        &self, features: wgt::Features,
    ) -> Vec<&'static CStr>;
    //   ^ The list VolcanoPlot must include in vkCreateDevice
    //     (superset OK per contract at :2566).

    // adapter.rs:2570 — wrap a foreign ash::Device
    pub unsafe fn device_from_raw(
        &self,
        raw_device: ash::Device,
        drop_callback: Option<DropCallback>,
        enabled_extensions: &[&'static CStr],
        features: wgt::Features,
        limits: &wgt::Limits,
        memory_hints: &wgt::MemoryHints,
        family_index: u32,
        queue_index: u32,
    ) -> Result<OpenDevice<Api>, DeviceError>;
}

// wgpu/src/api/adapter.rs:77
impl wgpu::Adapter {
    pub unsafe fn create_device_from_hal<A: hal::Api>(
        &self,
        hal_device: OpenDevice<A>,
        desc: &DeviceDescriptor,
    ) -> Result<(wgpu::Device, wgpu::Queue), RequestDeviceError>;

    // adapter.rs:131 — also useful for diagnostics
    pub unsafe fn as_hal<A: hal::Api, F, R>(f) -> Option<R>;
}
```

**This is the lynchpin**: `device_from_raw` takes a foreign
`ash::Device`, the extensions/features it was created with, and the
queue family/index wgpu should grab. wgpu then treats it as its own.

### 4.3 Device / Queue raw access

```rust
// wgpu/src/api/device.rs:668
impl wgpu::Device {
    pub unsafe fn as_hal<A: hal::Api, F, R>(&self, f: F) -> Option<R>
    where F: FnOnce(Option<&A::Device>) -> R;
    // → hal::vulkan::Device → .raw_device() -> &ash::Device   (vulkan/mod.rs:1520)
    //                        → .raw_queue()  -> vk::Queue     (vulkan/device.rs:834)
}

// wgpu/src/api/queue.rs:360 — same pattern for Queue
```

### 4.4 Texture import

```rust
// wgpu-hal/src/vulkan/device.rs:296
impl hal::vulkan::Device {
    pub unsafe fn texture_from_raw(
        &self,
        vk_image: vk::Image,
        desc: &hal::TextureDescriptor,      // wgpu desc (format/size/usage)
        drop_callback: Option<DropCallback>,
        memory: TextureMemory,               // External for our case
    ) -> hal::vulkan::Texture;
    // Contract (from doc comments):
    //  - vk_image created respecting desc
    //  - TextureMemory::External → wgpu does NOT own/track the memory
    //  - drop_callback None → wgpu destroys the image on drop (we want
    //    Some(...) or External+keep-alive so VP controls lifetime)
}

// wgpu/src/api/device.rs:341
impl wgpu::Device {
    pub unsafe fn create_texture_from_hal<A: hal::Api>(
        &self, hal_texture: A::Texture, desc: &TextureDescriptor,
    ) -> wgpu::Texture;
}
```

And the buffer twin used by Phase 1:

```rust
// wgpu-hal/src/vulkan/mod.rs:743-769
impl hal::vulkan::Buffer {
    pub unsafe fn from_raw(vk_buffer: vk::Buffer) -> Self;
    pub unsafe fn from_raw_externally_owned(vk_buffer: vk::Buffer) -> Self;
    pub unsafe fn from_raw_managed(vk_buffer: vk::Buffer, ...) -> Self;
}

// wgpu/src/api/device.rs:463
impl wgpu::Device {
    pub unsafe fn create_buffer_from_hal<A: hal::Api>(
        &self, hal_buffer: A::Buffer, desc: &BufferDescriptor,
    ) -> wgpu::Buffer;
}
```

### 4.5 eframe injection

```rust
// egui-wgpu-0.37/src/setup.rs:326
pub struct WgpuSetupExisting {
    pub instance: wgpu::Instance,
    pub adapter:  wgpu::Adapter,
    pub device:   wgpu::Device,
    pub queue:    wgpu::Queue,
}

// NativeOptions.wgpu_options = WgpuConfiguration {
//     wgpu_setup: WgpuSetup::Existing(WgpuSetupExisting{..}), ...
// }
```

So eframe can run entirely on *our* wrapped device — same VkDevice
VolcanoPlot renders on.

### 4.6 Completion callback (slot lifecycle)

```rust
impl wgpu::Queue {
    pub fn on_submitted_work_done(&self, callback: impl FnOnce() + Send + 'static);
}
```

Used to release frame slots back to VolcanoPlot once wgpu's use of a
given image has retired on the GPU.

### 4.7 ash version

`ash 0.38.0+1.3.281` is already in the lockfile (via wgpu-hal). The C++
side passes raw `VkDevice`/`VkImage`/`VkSemaphore` handles across FFI
as `u64`; Rust reconstructs `ash::Device` with
`ash::Device::load(&instance_fp, vk_device)` — **must** be loaded from
the same `ash::Entry`/instance function table we wrap, not a second
`Entry::load()`.

---

## 5. Synchronization design

This is the part most likely to be got subtly wrong. Three sub-problems:
producer→consumer ordering, consumer→producer slot release, resize.

### 5.1 Frame slots

`N = 2` frame slots (image + semaphore pair each); upgrade to 3 if
pipelining shows bubbles. Slot state machine:

```
   Free ──render submitted──► Rendering ──submit returned──► ReadyToSample
      ◄────────────────────┐                                    │
      │                    │ wgpu sampled +                     │ consumed by
      │     release sent   │  on_submitted_work_done fired      │ UI (wait-injected)
      └────────────────────┴────────────────────────────────────┘
```

- `Free`/`Rendering`/`ReadyToSample` tracked on the worker side.
- `ReadyToSample → Free` driven by the wgpu completion callback, never
  by time or frame counts.

### 5.2 Producer → consumer: semaphore + wait-injection

Per slot `i`, created once at surface init on the shared device:

```cpp
// C++ (VP side), per slot
vk::SemaphoreCreateInfo sci{};
renderDoneSem_[i]  = device.createSemaphoreUnique(sci);
```

Render submit (worker thread):

```cpp
vk::SubmitInfo si{};
si.setCommandBuffers(cmd[i]);
si.setSignalSemaphores(renderDoneSem_[i].get());
graphicsQueue.submit(si);            // NO fence needed for display path
```

Consumer side (UI thread, before egui submits this frame — i.e., inside
`eframe` paint, or earlier at the point we register the texture):

```rust
// One vkQueueSubmit with an empty command list but a wait semaphore,
// issued on wgpu's raw queue BEFORE wgpu's own submissions this frame.
// Vulkan queue semantics: execution order == submission order, so the
// wait resolves before any of egui's work samples the image.
device.as_hal::<wgpu_hal::api::Vulkan, _, _>(|hal_dev| {
    let hal_dev = hal_dev.unwrap();
    let raw: &ash::Device = hal_dev.raw_device();
    let q = hal_dev.raw_queue();
    let wait = slot.wait_sem;   // same VkSemaphore handle, imported below
    let dst_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
    let submit = vk::SubmitInfo::default()
        .wait_semaphores(std::slice::from_ref(&wait))
        .wait_dst_stage_mask(std::slice::from_ref(&dst_stage));
    raw.queue_submit(q, std::slice::from_ref(&submit), vk::Fence::null())
}).unwrap()
```

Because both semaphores and the queue live on **one VkDevice**, no
`VK_KHR_external_semaphore_fd` import is needed at all — the
`VkSemaphore` handle VP signals is usable verbatim by wgpu's queue.

**Ordering caveat**: the wait-injection must happen-before wgpu's
`queue.submit` for this frame *in queue submission order*. Since both
come from the UI thread timeline — we inject inside `App::update` /
paint, and eframe submits during the same vsync slot afterwards — this
holds. If eframe ever submits from another thread, add an explicit
`queue_submit`-after-`inject` ordering contract or move injection into
a `egui_wgpu::Callback` prepass.

**Alternative, even simpler**: submit VP's command buffer **on wgpu's
raw queue** (same device ⇒ same queue usable by anyone holding it).
Then ordering is literally submission order — inject VP's submit from
the UI thread before egui's, no semaphores at all. Chosen as the
fallback inside Option A if semaphore injection misbehaves; the
semaphore variant keeps VP's own queue/parallelism and is preferred.

### 5.3 Consumer → producer: `on_submitted_work_done`

Each frame the UI samples slot `i`, it attaches a release:

```rust
queue.on_submitted_work_done(move || {
    // Runs when all GPU work submitted before this call has retired.
    // Slot i's image is no longer sampled — safe for VP to re-render.
    slot_release_tx.send(i);   // → worker thread picks it up
});
```

VP must never re-record commands targeting slot `i` while a previous
frame's wgpu submissions might still sample it — including texture
views egui holds in its internal bind groups, which retire with the
submission, not the `wgpu::Texture` handle. `on_submitted_work_done`
covers both.

### 5.4 The "who waits" edge case

If the worker renders faster than the UI displays (it will, GPU-bound):
both slots end up `Rendering/ReadyToSample`; the worker blocks on a
condvar until a release arrives — same back-pressure the current
`Poll::Busy` one-deep pipeline already gives. Frame pacing stays
UI-driven.

If the UI is faster: it displays the newest `ReadyToSample` slot, or
the last sampled one if none is ready — equivalent to today's
"previous texture stays visible" semantics. No tearing possible: a
slot being sampled is never being rendered.

### 5.5 Layout management

Render target images are created `eGeneral`-capable or transitioned
per frame. Two viable layouts:

- `VK_IMAGE_LAYOUT_GENERAL` everywhere: VP renders into it, wgpu
  samples it, zero transitions. Costs a little render performance on
  some tilers; simplest correctness-wise. **Recommended initially.**
- Proper transitions: `UNDEFINED→COLOR_ATTACHMENT_OPTIMAL` (VP begin),
  `COLOR_ATTACHMENT_OPTIMAL→SHADER_READ_ONLY_OPTIMAL` (VP end, with the
  renderDone semaphore making the transition visible to wgpu's queue —
  same device ⇒ layout is device-global, safe).
  wgpu requires `texture_from_raw` `desc.usage` to include
  `TEXTURE_BINDING`; wgpu's own layout tracking assumes
  `TextureUses::RESOURCE`-compatible state. Use `desc` with
  `TextureUsages::TEXTURE_BINDING | COPY_SRC | COPY_DST` and let wgpu
  treat it as already in sampling state. Re-transition back inside
  VP's next render for that slot.

Phase 1 (shared buffer) avoids the question entirely — buffer needs no
layout. Do Phase 1 first, get synchronization right there, then swap
the payload for an image.

### 5.6 Resize

`fig.resize(w,h)` today rebuilds the backend on the worker. Under
sharing:

1. Worker: stop submitting; drain (wait renderDone + slot releases).
2. Destroy slot images/semaphores AND the wgpu-side `wgpu::Texture`
   wrappers — **drop order**: unregister `TextureId::User` from egui's
   `tex_manager` first (UI side, via a message), wait one
   `on_submitted_work_done`, then `texture_from_raw`'d textures drop
   (with `drop_callback = Some` so wgpu does not `vkDestroyImage`
   images VP owns — see contract at §4.4: pass
   `TextureMemory::External` and keep memory alive until callback).
3. VP recreates images at the new size.
4. Rust re-wraps, re-registers texture id, resumes.

During steps 1–4 the UI keeps painting egui chrome — no freeze; the
scope area shows the last texture or a placeholder. This preserves the
"resize off the UI thread" property already implemented.

### 5.7 Teardown ordering

- eframe owns the `wgpu::Device` wrapper; VP owns the raw VkDevice.
- Contract: all `texture_from_raw`/`create_texture_from_hal` products
  and the wgpu device/queue must drop before `VpFigure` destroys the
  raw device. eframe drops its RenderState at app exit *after*
  `App::on_exit` — so `LiveplotApp::on_exit` must signal the worker to
  stop and join it **before** returning, then drop `VolcanoSurface`,
  and only then may wgpu/eframe teardown proceed. Since VP created the
  VkDevice, VP destroys it — but only after wgpu's wrappers are gone.
  Achieve this by keeping an `Arc<DeviceGuard>` shared: Rust holds a
  guard that prevents VP's `vkDestroyDevice` until the last wgpu
  reference releases; the worker joins before `eframe` exits.
  Detail: `WgpuSetupExisting` devices are dropped inside eframe's
  shutdown — verify drop order empirically in Phase 0 (§9.1).

---

## 6. C++ / FFI changes (vendor/VolcanoPlot)

Files:

- `include/volcano/ffi/volcano_ffi.h` — new exports
- `src/ffi/volcano_ffi.cpp` — implementations
- `src/core/Device.cpp` — extension/feature injection
- `src/backend/HeadlessBackend.cpp` — exportable render target +
  per-slot semaphores
- `src/render/Renderer.cpp` — submit with slot semaphores; expose
  last-rendered slot index

### 6.1 Device creation with extension union

`Device.cpp` currently builds `wanted` extension list from
`desc.extensions`. Add an FFI-driven injection point:

```cpp
// DeviceDesc gets:
std::vector<std::string> extraDeviceExtensions;   // from Rust/wgpu-hal
VkPhysicalDeviceFeatures2* extraFeatureChain = nullptr; // optional pNext
```

Rust computes `adapter.required_device_extensions(features)` **before**
device creation — chicken-and-egg solved by a two-pass bring-up:

1. Rust calls `vp_probe_device(&caps)` — C++ creates instance, picks
   physical device, returns: `vk_instance`, `vk_physical_device`,
   `api_version`, supported extension list.
2. Rust wraps instance+phd via `Instance::from_raw` + `expose_adapter`
   → asks wgpu-hal `required_device_extensions(features)` for the
   feature set eframe will request.
3. Rust calls `vp_create_shared_figure(&SharedCreateDesc{ required_exts
   (as strings), feature flags needed, w,h })` — C++ creates the
   VkDevice with `exts ∪ vp_exts`, enables the superset feature chain,
   returns `vk_device`, `graphics_family`, `queue_index`,
   `VkQueue` handle (wgpu's `family_index/queue_index` for
   `device_from_raw`).
4. Rust `device_from_raw(...)` + `create_device_from_hal` → inject
   into `WgpuSetupExisting` → `eframe::run_native`.

Extension list VP must union (RADV, wgpu 30, typical): wgpu's
`required_device_extensions` output — usually
`VK_KHR_swapchain`, `VK_KHR_maintenance*`, `VK_EXT_image_robustness`,
`VK_KHR_timeline_semaphore` etc. depending on features — plus VP's own
(none beyond core today; keep `VK_KHR_external_*` out — not needed for
same-device sharing).

**Feature chain caveat**: `device_from_raw` takes `features` — the
foreign device must actually have them enabled. wgpu 30 on Vulkan
typically needs `features2` basics only; if eframe requests more
(e.g. `TIMESTAMP_QUERY`), VP's `vkCreateDevice` must enable them via a
`VkPhysicalDeviceFeatures2` pNext chain. Pass the whole
`VkPhysicalDeviceVulkan11/12/13Features` set computed on the Rust side
by querying wgpu's required features, serialized as a bitmask blob
across FFI (simplest: a fixed `VpDeviceFeatures` struct covering the
fields wgpu 30 can request).

### 6.2 Exportable render targets + semaphores

HeadlessBackend gets a **slot mode**:

```cpp
struct VpFrameSlot {
    VkImage     image;        // color attachment, sampled later by wgpu
    VmaAllocation alloc;
    VkImageView view;
    VkSemaphore render_done;  // signalled by VP's submit
};
// vp_create_figure gains flag; slots allocated at (w,h), format
// VK_FORMAT_R8G8B8A8_UNORM — keep parity with current RGBA readback
// format so egui's existing texture descriptor still matches.
```

- Image usage flags:
  `COLOR_ATTACHMENT | SAMPLED | TRANSFER_SRC | TRANSFER_DST`
  (`TRANSFER_SRC` keeps the CPU readback path working — needed for
  tests, dumps, and the fallback path).
- Same-device ⇒ no `VkExternalMemoryImageCreateInfo` needed.
- `vp_render_slot(fig) -> int` — renders into the next free slot,
  submits signalling `render_done[i]`, returns `i`. Non-blocking
  (fence optional — completion is tracked by the semaphore + wgpu
  callbacks, not by CPU waits; keep an optional fence for tests).
- `vp_slot_handles(fig, i, &VpSlotHandles{ image, semaphore })` — raw
  `u64` handles for Rust.
- `vp_slots_resize(fig, w, h)` — drain + recreate (§5.6).
- Keep `vp_render_rgba`/`vp_render_rgba_view` untouched for the
  fallback path and the entire FFI test suite — zero test churn.

### 6.3 Submit path

`Renderer::renderFrame` currently submits once with a fence. Slot mode
adds `renderFrameInto(slot)`:

- record into per-slot cmd buffer (or reuse transient pool — fine,
  commands may be re-recorded each frame)
- end with optional `COLOR_ATTACHMENT→SHADER_READ_ONLY` transition
- `queue.submit({cmd}, signal=render_done[slot])`
- no fence wait (Phase 2 display path); tests may still pass a fence.

Keep the existing fence+readback submit for `vp_render_rgba*` —
orthogonal code path, same prepare.

---

## 7. Rust changes (liveplot-rs)

Files:

- `src/volcano/mod.rs` — FFI declarations + `SharedDeviceInfo`
- `src/volcano/shared.rs` — NEW: hal glue (~400 lines, all `unsafe`
  concentrated here)
- `src/volcano/surface.rs` — slot state machine, wait-injection,
  texture registration
- `src/app/liveplot_app.rs` — eframe setup path, `on_exit` ordering
- `Cargo.toml` — `wgpu`, `wgpu-hal`, `ash` move from transitive to
  direct deps (pin `wgpu-hal = "=30.0.1"` matching eframe's wgpu)

### 7.1 Bring-up sequence (new file `shared.rs`)

```rust
pub struct SharedGraphics {
    pub instance: wgpu::Instance,
    pub adapter:  wgpu::Adapter,
    pub device:   wgpu::Device,
    pub queue:    wgpu::Queue,
    // Raw handles kept for the lifetime of the app:
    pub vk_instance: ash::Instance,
    pub vk_device:   ash::Device,   // == VP's VkDevice, wrapped
    pub raw_queue:   vk::Queue,
    pub family_index: u32,
}

pub unsafe fn init_shared(vp_probe: VpDeviceProbe) -> Result<SharedGraphics> {
    // 1. ash::Entry::load — the ONLY entry; both halves share it.
    let entry = ash::Entry::load()?;
    // 2. Wrap VP's instance.
    let ash_inst = ash::Instance::load(
        entry.static_fn(),
        vk::Instance::from_raw(vp_probe.vk_instance));
    let hal_inst = hal::vulkan::Instance::from_raw(
        entry, ash_inst.clone(), vp_probe.api_version,
        vp_probe.enabled_instance_exts(), /* flags */ 0,
        /* has_nv_optimus */ false,
        Some(Box::new(|_| { /* no-op: VP owns instance */ })),
        None)?;
    let wgpu_inst = wgpu::Instance::from_hal::<hal::api::Vulkan>(hal_inst);
    // 3. Expose VP's physical device as an adapter.
    let hal_exposed = wgpu_inst
        .as_hal::<hal::api::Vulkan, _, _>(|i| {
            i.unwrap().expose_adapter(vk::PhysicalDevice::from_raw(
                vp_probe.vk_physical_device))
        }).flatten().ok_or(anyhow!("wgpu rejects VP physical device"))?;
    let adapter = wgpu_inst
        .create_adapter_from_hal::<hal::api::Vulkan>(hal_exposed.adapter);
    // 4. Ask what the device must have, hand it back to C++ (second
    //    FFI call creates the actual VkDevice — see §6.1).
    // 5. device_from_raw:
    let open = hal_exposed.adapter.device_from_raw(
        vk_device_from_ffi, None /* VP owns vkDestroyDevice? NO —
            wgpu must be able to destroy ITS wrapper; use drop_callback
            that forwards to vp's destroy so ordering is right */,
        &enabled_exts, features, &limits, &memory_hints,
        family_index, 0)?;
    let (device, queue) = adapter
        .create_device_from_hal::<hal::api::Vulkan>(open, &desc)?;
    Ok(SharedGraphics { .. })
}
```

Wait — careful: `device_from_raw` on the *hal adapter* requires the
adapter object from `expose_adapter`, which lives inside the wgpu
instance context; ordering: instance → adapter → **then** device
creation on C++ side → then `device_from_raw`. The C++ VkDevice must
exist before step 5, and its extension list is known only after step 4.
So the probe returns the phd **without** a device; `vp_create_shared_figure`
happens between steps 4 and 5. Figure creation currently implies
backend+device creation — split it: `vp_probe` (instance+phd) →
`vp_create_shared_figure` (device+backend+figure). The normal
`vp_create_figure` keeps its one-shot behavior for tests/headless.

### 7.2 eframe wiring

```rust
let native_options = eframe::NativeOptions {
    wgpu_options: egui_wgpu::WgpuConfiguration {
        wgpu_setup: WgpuSetup::Existing(WgpuSetupExisting {
            instance: shared.instance,
            adapter:  shared.adapter,
            device:   shared.device.clone(),
            queue:    shared.queue.clone(),
        }),
        ..Default::default()   // verify: present_mode, format prefs
    },
    ..Default::default()
};
```

`eframe::run_native` then uses the wrapped device for everything —
swapchain included. The swapchain surfaces are created by wgpu on
VP's `VkDevice` — requires `VK_KHR_swapchain` in VP's extension list
(it'll be in `required_device_extensions` output once eframe's
requested features are passed) and a `VkSurfaceKHR` created through
wgpu's instance — wgpu handles that internally (`wgpu-hal` surface
creation on the raw instance works regardless of who created it).

**This is the boldest part**: egui's entire presentation runs on
VolcanoPlot's device. If `expose_adapter` rejects the phd or
`create_device_from_hal` fails → fall back: let eframe create its own
device normally, VP keeps its own → Option B/CPU path (§11, §12).

### 7.3 Per-frame flow (worker + UI)

Worker thread (`surface.rs`):

```rust
// per job:
let slot = slots.acquire_free()?;          // condvar if both busy
vp_trace_slide_data_f64(...);              // unchanged
let i = vp_render_slot(fig, slot);         // submit, signal sem[i]
pending_ready.push(i);                     // mailbox to UI
```

UI thread (`update()` in the Volcano overlay path):

```rust
for i in pending_ready.drain() {
    // 1. wrap/refresh the wgpu texture for this slot (once per size
    //    generation — texture wrap is stable across frames!)
    // 2. inject wait on raw queue
    inject_wait(&shared, slot.semaphore(i))?;
    // 3. register (only first time per slot generation)
    tex_manager.set(TextureId::User(slot_tex_id(i)), view);
    queue.on_submitted_work_done(move || release_tx.send(i));
    // 4. remember `displayed_slot = i` for the Image widget
}
scope.painter().image(displayed_slot, rect, uv, tint);
```

Note: the `wgpu::Texture` wrapper is created **once per slot per
resize-generation**, not per frame — `texture_from_raw` is cheap but
the egui texture registration churn is not free. TextureId stays
constant per slot; the wait-injection is what changes each frame.

### 7.4 `unsafe` audit surface

Everything `unsafe` lives in `shared.rs` behind `init_shared`,
`wrap_slot_image`, `inject_wait`, ~150–250 lines. Invariants to
document:

- ash objects loaded once from one `Entry`.
- `enabled_extensions` passed to `device_from_raw` == the list passed
  to `vkCreateDevice` (assert by string compare at init).
- `TextureMemory::External` + `drop_callback` — wgpu must never
  `vkDestroyImage` a VP-owned image (or if it does, it must outlive
  the C++ object — pick `drop_callback` that defers to VP's
  destruction callback; simplest: keep `wgpu::Texture` alive for the
  surface lifetime and drop explicitly before `VpFigure`).
- Semaphore waits injected only on `raw_queue` belonging to the same
  `VkDevice` that signals them.
- No `wgpu` objects outlive `VpFigure` (§5.7 guard).

---

## 8. Phases & milestones

### Phase 0 — Spike: prove `device_from_raw` (½–1 day)

Standalone binary (`examples/hal_probe.rs` or a `#[test]`):

1. `vp_probe` → wrap instance → `expose_adapter` → print
   `adapter.get_info()`.
2. Compute `required_device_extensions(FEATURES_EFRAME_USES)` —
   where `FEATURES_EFRAME_USES` = what eframe requests today
   (read `WgpuConfiguration` defaults: `supported_features`/`limits`).
3. `vp_create_shared_figure` with that ext list → `device_from_raw` →
   `create_device_from_hal` → `queue.submit([])` trivially →
   `device.poll(Wait)` — proves wgpu accepts the foreign device.
4. Then `eframe::run_native` with `WgpuSetupExisting` showing an empty
   egui window.

Gate: if step 2–4 fails, Option A is dead → escalate to Option B
(dma-buf plan §12) or abandon. **This gates everything; do it first.**

### Phase 1 — Shared buffer zero-CPU-copy (1–2 days)

- C++: `vp_render_slot` + after render `vkCmdCopyImageToBuffer` into a
  per-slot VkBuffer (same device, `TRANSFER_DST`, linear RGBA).
- Rust: `create_buffer_from_hal` wraps each slot buffer → each frame:
  encoder `copy_buffer_to_texture` → `wgpu::Texture` (created by wgpu,
  normal texture!) → `TextureId::User`.
- Sync: `render_done[i]` semaphore + wait-injection before the
  copyBufferToTexture encoder submits — the wait covers the copy AND
  egui's draw since both go on the same queue after injection.
- Release: `on_submitted_work_done`.

CPU cost after Phase 1: memcpy + write_texture gone; cost ≈
worker record/submit ~2.5 ms + trivial encoder. **Expected ~22–27 %
total (from 42)**. This is already most of the win.

### Phase 2 — Direct image wrap (1–2 days)

- VP render target IS the sampled texture (no buffer copy):
  `texture_from_raw` per slot per generation, `eGeneral` layout first,
  proper transitions second.
- Remove the per-frame `copy_buffer_to_texture` — pure semaphore +
  sample.
- CPU → ~15–20 % target; worker is submit-only plus `prepare`.

### Phase 3 — Hardening (1 day)

- Resize/drain path (§5.6), teardown order (§5.7), fallback matrix
  (§11), `VP_SHM=off` env kill-switch, metrics
  (`LIVEPLOT_DEBUG_PERF` gains shm-path timing).

### Phase 4 — Validation & docs

- All 26 `volcano_ffi` tests unchanged (CPU readback path intact —
  the suite never touches shared path; add ONE new test exercising
  `vp_render_slot` + semaphore).
- e2e suite under `fft` feature — screenshots through the shm path.
- Stress benchmark table identical to §0 — CPU/GPU/fps/RSS before/after.
- Update `docs/volcano-integration.md`, `AGENTS.md` notes.

### Phase 5 — Optional follow-ups

- Triple-buffer if bubbles observed.
- `VK_IMAGE_LAYOUT` proper transitions (from `eGeneral`) if profiling
  shows the General layout hurting VP's render.
- Windows/macOS: Option B (dma-buf on Linux is POSIX-only; Windows
  wants `VK_KHR_external_memory_win32` NT handles — separate project;
  macOS/Metal → keep CPU path or port via IOSurface — out of scope).

---

## 9. Detailed work items

### 9.1 Phase-0 checklist (exact)

| # | Item | File | Acceptance |
|---|------|------|-----------|
| 0.1 | `vp_probe_device` FFI: create instance+phd, no device | `volcano_ffi.{h,cpp}` | returns handles struct |
| 0.2 | ash wrap of foreign instance | `shared.rs` | `Instance::from_raw` Ok |
| 0.3 | `expose_adapter(phd)` | `shared.rs` | `Some(_)`, name matches RADV |
| 0.4 | required ext list printed | `shared.rs` | log line, e.g. swapchain+maintenance |
| 0.5 | `vp_create_shared_figure(exts, features)` creates VkDevice | `volcano_ffi.cpp`, `Device.cpp` | vk_device valid |
| 0.6 | `device_from_raw` + `create_device_from_hal` | `shared.rs` | `(device, queue)` Ok |
| 0.7 | trivial wgpu submit + poll | `shared.rs` | no validation errors |
| 0.8 | eframe on `WgpuSetupExisting` | `examples/hal_probe.rs` | window opens, egui renders |
| 0.9 | drop-order experiment: exit cleanly | — | no validation teardown errors |

Feature flags eframe 0.37 requests by default (verify against
`setup.rs` `WgpuConfiguration::default()`): typically
`supported_features: Features::default()`, `limits` via
`Limits::default().using_resolution(...)` on adapter. Pass the same
into `required_device_extensions`.

### 9.2 Phase-1 checklist

| # | Item | File | Acceptance |
|---|------|------|-----------|
| 1.1 | `VpFrameSlot` (image+buffer+sem) ×2 in HeadlessBackend | `HeadlessBackend.{h,cpp}` | created at fig size |
| 1.2 | `vp_render_slot` + `copyImageToBuffer` per slot | `volcano_ffi.cpp` | returns slot idx |
| 1.3 | `vp_slot_handles` export | `volcano_ffi.{h,cpp}` | handles struct |
| 1.4 | `create_buffer_from_hal` wrap | `shared.rs` | wgpu::Buffer over VkBuffer |
| 1.5 | per-slot wgpu texture + `copy_buffer_to_texture` encoder | `surface.rs` | texture updates |
| 1.6 | wait-injection fn | `shared.rs` | `queue_submit` on raw_queue |
| 1.7 | slot state machine + `on_submitted_work_done` release | `surface.rs` | no slot starvation at 60fps |
| 1.8 | `TextureId::User` per slot; Image widget swap | `scope` overlay | displays |
| 1.9 | benchmark vs §0 | stress_demo | CPU ≤ ~28% at same workload |

### 9.3 Phase-2 checklist

| # | Item | File | Acceptance |
|---|------|------|-----------|
| 2.1 | slot images `SAMPLED` usage + `eGeneral` or transitions | `HeadlessBackend.cpp` | — |
| 2.2 | `texture_from_raw` + `create_texture_from_hal` | `shared.rs` | wgpu::Texture per slot |
| 2.3 | register wrapped texture directly (skip copy) | `surface.rs` | identical pixels to Phase 1 |
| 2.4 | remove per-frame copyBufferToTexture | `surface.rs` | CPU ≤ ~20% |
| 2.5 | validation layers clean run | — | 0 errors over 60 s stress |

### 9.4 Phase-3 checklist

| # | Item | File | Acceptance |
|---|------|------|-----------|
| 3.1 | resize drain + rewrap | `surface.rs` | sidebar drag: no freeze/crash |
| 3.2 | `on_exit` join + DeviceGuard drop order | `liveplot_app.rs` | clean exit, val layers quiet |
| 3.3 | `VP_SHM=0` env → old path | `surface.rs` | identical old behavior |
| 3.4 | wgpu-backend ≠ Vulkan → CPU path | `liveplot_app.rs` | GL/Wayland fallback works |
| 3.5 | metrics: per-frame ns breakdown | perf probe | report in bench table |

### 9.5 Phase-4 checklist

- `tests/volcano_ffi.rs`: new `volcano_slot_render_signals_semaphore`
  (creates shared figure, renders slot, `vkWaitForFences`-equivalent:
  poll semaphore via exported handle in test — proves GPU submit +
  signal ordering independent of wgpu).
- e2e: `xy_scope` + `volcano_parity` run under shm mode (env on);
  compare `LIVEPLOT_DEBUG_DUMP` frames bitwise vs CPU path (should be
  identical — same image content).
- Stress: rerun §0 table; publish before/after.
- README/AGENTS updates.

---

## 10. Test plan

### 10.1 Unit / FFI level

| Test | Path | Asserts |
|------|------|---------|
| `volcano_slot_render_signals_semaphore` | `tests/volcano_ffi.rs` | render_done semaphore reaches signaled state within 2 s; slot image non-empty via CPU readback of same slot |
| `volcano_slot_double_buffer_no_tear` | `tests/volcano_ffi.rs` | alternate 2 slots × 200 renders; each slot's content matches its generation (stamp a color in test pattern) |
| `volcano_shared_probe_reports_wgpu_exts` | `tests/volcano_ffi.rs` | probe→create_shared roundtrip, exts accepted |

### 10.2 Rust integration

- `surface.rs` slot state machine unit tests (mock semaphores/slots —
  abstract handles behind a trait so pure-Rust tests don't need Vulkan).
- Wrap/unwrap generation counter tests (resize → old textures invalid).

### 10.3 E2E

- Existing e2e suite (`--features fft`) must pass with `VP_SHM=1` —
  pixel-parity assertions exercise the same framebuffer through the
  new path.
- New e2e: 60-frame capture under shm, assert no tearing — checker:
  render frames alternating a sentinel color via a test trace; scan
  dumped frames for mixed-color scanlines.

### 10.4 Validation layers

- `VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation` full stress run:
  **0 errors** is the bar (warnings about foreign-image usage in wgpu
  are expected; enumerate and whitelist in docs).

### 10.5 Performance acceptance

| Metric | Before (§0) | Phase-1 target | Phase-2 target |
|--------|-------------|----------------|----------------|
| CPU %  | 43.4        | ≤ 28           | ≤ 20           |
| Worker % | ~26       | ≤ 14           | ≤ 10           |
| UI %   | ~12.6       | ≤ 9            | ≤ 9            |
| fps    | ~52         | ≥ 52           | ≥ 52           |
| RSS    | 335 MB      | ≈ same         | −5–10 MB (staging) |

Measured with the same `/tmp/run_stress.sh` methodology (40 s window,
12 s warmup, pidstat + `gpu_busy_percent`).

---

## 11. Risk register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| `expose_adapter` rejects VP's phd (missing inst ext e.g. `VK_KHR_get_physical_device_properties2` — wgpu REQUIRES it) | Low | blocker | probe returns supported exts; enable wgpu's required instance exts in `vp_probe` (superset fine) |
| `device_from_raw` wants features VP didn't enable | Medium | blocker | feature-chain pass-through in §6.1; worst case restrict eframe `supported_features` to a safe subset |
| wgpu submits from another thread → wait-injection ordering breaks | Low-Med | tear/corrupt | inject inside egui paint callback (ordered before surface present) rather than `update()` |
| `texture_from_raw` layout assumptions mismatch | Medium | corruption/validation spam | start `eGeneral`; verify with layers; escalate to explicit transitions |
| eframe drop order destroys wgpu device while VP alive | Medium | crash on exit | DeviceGuard refcount (§5.7); verify in Phase 0.9 |
| Slot starvation deadlock (worker waits free slot, UI waits frame) | Low | hang | N≥2 slots + release on `on_submitted_work_done`; timeout→recreate |
| Validation-layer noise on foreign image | Medium | none (cosmetic) | whitelist, document |
| Driver bugs w/ same-device multi-queue semaphores | Low | stall | fallback: submit VP work on wgpu's raw_queue (§5.2 alt) |
| wgpu upgrade (eframe bump) breaks hal API | High (over time) | build break | pin `wgpu-hal = "=30.0.1"`; single `shared.rs` contains the blast radius |
| Non-Linux: ext set differs | Medium | platform-specific | keep `VP_SHM` platform-gated `#[cfg(target_os="linux")]`; others → CPU path |

---

## 12. Fallback: Option B (cross-device dma-buf) sketch

Recorded for completeness — implement ONLY if Phase 0 kills Option A.

1. Both devices keep existing creation paths.
2. Match physical devices: compare `VkPhysicalDeviceProperties.deviceID`
   + `vendorID` + `VK_KHR_external_memory` UUID
   (`VkPhysicalDeviceIDProperties.deviceUUID`) on both sides; abort →
   CPU path if different GPUs.
3. VP images created with
   `VkExternalMemoryImageCreateInfo{handleTypes=DMA_BUF_BIT}` +
   `VkExportMemoryAllocateInfo`; `vkGetMemoryFdKHR` → dma-buf fd;
   export `VkImageDrmFormatModifierPropertiesEXT.modifier`.
4. Rust: `ash::Device::create_image` on wgpu's raw device with matching
   modifier via `VkImageDrmFormatModifierExplicitCreateInfoEXT`, alloc
   with `VkImportMemoryFdInfoKHR{fd}` → `texture_from_raw`.
5. Semaphores: `SYNC_FD` exportable binary semaphore per slot;
   `vkGetSemaphoreFdKHR` post-submit → `vkImportSemaphoreFdKHR` into
   consumer-side semaphore → wait-injection as §5.2.
   sync_fds are one-shot ⇒ fresh import per frame per slot; semaphores
   themselves are recycled after re-creation with
   `VkSemaphoreImportFlags::TEMPORARY`.
6. fd lifecycle: `dup()` before import (import consumes); close after.
   No `SCM_RIGHTS` needed — same process, fds passed via the worker→UI
   channel as integers.
7. Estimated effort: 2–3× Option A; benefit: eframe keeps its own
   device (works when VP's device can't host swapchain, e.g. missing
   surface ext on a compute-only device).

Plan-B-only extra risk: RADV modifier negotiation, `VK_EXT_queue_family_foreign`
ownership transfers between queues of different devices
(`VK_QUEUE_FAMILY_FOREIGN_EXT` acquire/release barriers in the submit
stream).

---

## 13. Interim cheap wins orthogonal to this plan

These ship regardless and don't conflict:

- `LIVEPLOT_VOLCANO_MAX_RENDER_HZ` — already landed (30 Hz cap → 38 % CPU).
- `VP_MSAA` knob — landed; no gain on this iGPU but keep.
- Buffer headroom for sliding appends — landed.
- Reduce readback size: render at `scope_rect` size instead of full
  figure (already true) — verify scope rect == figure size, else clamp.
- Skip `ColorImage` alloc: reuse a pooled `ColorImage` per size — tiny
  win, do while touching `surface.rs` anyway.

---

## 14. File-touch summary

| File | Change |
|------|--------|
| `vendor/…/ffi/volcano_ffi.h` | `vp_probe_device`, `vp_create_shared_figure`, `vp_render_slot`, `vp_slot_handles`, `VpDeviceProbe`, `VpSlotHandles` |
| `vendor/…/ffi/volcano_ffi.cpp` | impls above; keep `vp_render_rgba*` intact |
| `vendor/…/core/Device.cpp` | ext/feature injection in `vkCreateDevice` |
| `vendor/…/backend/HeadlessBackend.{h,cpp}` | `VpFrameSlot` array, slot resize |
| `vendor/…/render/Renderer.{h,cpp}` | `renderFrameInto(slot)`, signal sem |
| `src/volcano/shared.rs` | NEW — all hal glue + unsafe |
| `src/volcano/mod.rs` | FFI decls, `SharedDeviceInfo` re-export |
| `src/volcano/surface.rs` | slot state machine, injection, tex registration |
| `src/app/liveplot_app.rs` | `WgpuSetupExisting` wiring, exit ordering |
| `Cargo.toml` | +`wgpu`, `wgpu-hal="=30.0.1"`, `ash="0.38"` |
| `tests/volcano_ffi.rs` | +3 slot tests |
| `docs/volcano-integration.md` | architecture update |

Estimated net new code: ~900–1200 lines Rust (mostly `shared.rs` +
state machine), ~400–600 lines C++, ~150 lines FFI plumbing.

---

## 15. Open questions to resolve in Phase 0

1. Does `expose_adapter` require the instance be created by wgpu's own
   `Instance::init`? (Source reading says no — `from_raw` +
   `expose_adapter` is a supported combo, used by `gfx-rs` embedders —
   but verify empirically first.)
2. Which exact `enabled_extensions` list does wgpu 30 require on this
   RADV? (Print at probe time; union into `vp_create_shared_figure`.)
3. Does eframe 0.37's `WgpuSetupExisting` path still create the
   surface/swapchain itself? (Yes per setup.rs, but confirm the
   swapchain ext lands in `required_device_extensions` output —
   surface support is adapter-level, swapchain is device-level.)
4. Can `texture_from_raw`'d textures feed egui's `tex_manager.set`
   directly, or does egui-wgpu require textures it allocated? (egui's
   `TextureId::User` path accepts arbitrary `wgpu::TextureView` —
   confirm `register_native_texture` accepts our wrapped view.)
5. Timeline vs binary semaphores for slot sync — binary chosen; if
   wgpu's queue uses timeline tracking internally and complains,
   switch.
6. Does `device_from_raw` require `queue_index 0` on the family wgpu
   grabs, and does that collide with VP's `getQueue(fam, 0)` — same
   family, separate `VkQueue` handles (`queue_index 0` vs `1`)? If the
   device was created with `queueCount ≥ 2` on that family both get
   distinct handles; else VP and wgpu SHARE `VkQueue` — which is legal
   (queue ops are externally synchronized — must mutex submits!) —
   decision: request `queueCount=2` for graphics family in
   `vp_create_shared_figure`, wgpu takes index 0, VP index 1. If
   family supports only 1 queue, serialize all submits through a
   shared `Mutex<vk::Queue>` accessed via `raw_queue()`.

---

## 16. Appendix A — current per-frame cost ledger (for reference)

```
component                              ms/frame CPU   notes
─────────────────────────────────────────────────────────────
get_points_ref + slide bookkeeping     ~0.3           UI thread
vp_trace_slide_data_f64 (8 traces)     ~0.1           worker
prepare (uploads+autoscale orchestr.)  ~1.5–5.9       worker, grew → fixed by headroom
record cmd + vkQueueSubmit             ~1.0–1.5       worker
fence sleep-poll                       ~0 (sleeps)    worker
staging→ColorImage memcpy 3.7MB        ~1.5           worker   ← KILLED by Phase 1
write_texture upload 3.7MB             ~1.0           UI       ← KILLED by Phase 1
egui repaint + tess                    ~5             UI (both backends)
producer                               ~0.5           producer
```

Post-Phase-2 predicted ledger: worker ≈ 2.5–3 ms, UI ≈ egui-only.
CPU% estimate: (2.5 + 5 + 0.5 + 0.4)/16.6 ≈ ~50 %×0.4 … measured
target ≤ 20 % total (the memcpy/upload were the ~18 % delta vs egui).

## 17. Appendix B — handle marshalling across FFI

```c
// volcano_ffi.h
typedef struct {
    uint64_t vk_instance;          // VkInstance
    uint64_t vk_physical_device;   // VkPhysicalDevice
    uint32_t api_version;          // vkEnumerateInstanceVersion
    uint32_t graphics_family;      // candidate family for wgpu
    uint32_t queue_count_graf;
} VpDeviceProbe;

typedef struct {
    uint64_t vk_device;            // VkDevice
    uint64_t graphics_queue0;      // for wgpu (family, index 0)
    uint32_t family_index;
    uint32_t queue_index;          // 0
} VpSharedDevice;

typedef struct {
    uint64_t image;                // VkImage (VP-owned)
    uint64_t semaphore;            // VkSemaphore render_done
    uint32_t width, height;
    uint32_t generation;           // resize epoch
} VpSlotHandles;

int  vp_probe_device(VpDeviceProbe* out);
int  vp_create_shared_figure(VpFigure** out,
        const char* const* required_exts, int n_exts,
        const uint8_t* feature_blob, size_t blob_len,
        int w, int h);
int  vp_shared_device(VpFigure*, VpSharedDevice* out);
int  vp_render_slot(VpFigure*, int* slot_out);        // -1 if none free
int  vp_slot_handles(VpFigure*, int slot, VpSlotHandles* out);
int  vp_slots_resize(VpFigure*, int w, int h);
void vp_slot_release_hint(VpFigure*, int slot);       // from wgpu cb
```

Generation counter in `VpSlotHandles` guards ABA: a stale `release`
from an older resize epoch is ignored by the state machine.

## 18. Appendix C — slot state machine (Rust pseudo-code)

```rust
enum SlotState { Free, Rendering, Ready, InUse }

struct SharedSlots {
    slots: Vec<Slot { image: vk::Image, sem: vk::Semaphore,
                      wgpu_tex: Option<wgpu::Texture>, state: SlotState,
                      generation: u32 }>,
    release_rx: Receiver<(usize /*slot*/, u32 /*gen*/)>,
    ready_tx:   Sender<usize>,   // worker → UI
}
// worker: acquire(Free→Rendering) → vp_render_slot → Ready → ready_tx
// UI:     Ready → inject_wait → sample → InUse
//         on_submitted_work_done → release_tx(slot,gen)
// worker: release_rx → InUse→Free (only if gen matches)
```

Deadlock analysis: worker blocks only when both slots `!Free`; releases
arrive exclusively from `on_submitted_work_done`, which wgpu fires for
every submitted frame — so releases arrive iff frames present, which is
iff the UI loop is alive. If the UI stalls >1 s (debugger), worker
times out its condvar and drops the job — same as today's Busy path.

## 19. Appendix D — Phase-0 spike code (complete sketch)

`examples/hal_probe.rs` — minimal standalone proof that a
VolcanoPlot-created `VkDevice` can host wgpu + eframe. This file is
throwaway-by-design; its code graduates into `src/volcano/shared.rs`.

```rust
//! Phase-0 spike: wgpu + eframe on VolcanoPlot's VkDevice.
//! Run: VP_SHM=1 cargo run --example hal_probe --features volcano

use ash::vk;
use liveplot::volcano::ffi;   // new exports from §6/§17

fn main() -> anyhow::Result<()> {
    // ── 1. Ask C++ to make instance + physical device (no VkDevice yet)
    let mut probe = ffi::VpDeviceProbe::zeroed();
    unsafe { ffi::vp_probe_device(&mut probe) }.check()?;

    // ── 2. Wrap the foreign instance in ash + wgpu-hal + wgpu
    let entry = unsafe { ash::Entry::load()? };
    let ash_inst = unsafe {
        ash::Instance::load(entry.static_fn(),
                            vk::Instance::from_raw(probe.vk_instance))
    };
    // Extensions the C++ instance was created with — returned by probe
    // (string list marshalled as part of VpDeviceProbe in real code).
    let inst_exts: Vec<&'static std::ffi::CStr> = probe.enabled_exts();
    let hal_inst = unsafe {
        wgpu_hal::vulkan::Instance::from_raw(
            entry,
            ash_inst.clone(),
            probe.api_version,
            0,                       // android_sdk_version / flags
            inst_exts,
            wgpu_hal::InstanceFlags::default()
                | wgpu_hal::InstanceFlags::VALIDATION,
            false,                   // has_nv_optimus
            Some(Box::new(|_| {})),  // drop cb: never destroy — VP owns
            None,                    // dx12 lib etc.
        )?
    };
    let instance = unsafe {
        wgpu::Instance::from_hal::<wgpu_hal::api::Vulkan>(hal_inst)
    };

    // ── 3. Expose VP's physical device to wgpu
    let exposed = unsafe {
        instance.as_hal::<wgpu_hal::api::Vulkan, _, _>(|i| {
            i.and_then(|i| i.expose_adapter(
                vk::PhysicalDevice::from_raw(probe.vk_physical_device)))
        })
    }.flatten().ok_or_else(|| anyhow::anyhow!("wgpu rejected phd"))?;
    let adapter = unsafe {
        instance.create_adapter_from_hal::<wgpu_hal::api::Vulkan>(
            exposed.adapter)
    };
    log::info!("adapter: {:?}", adapter.get_info());

    // ── 4. Negotiate the device contract
    let features = wgpu::Features::default();          // == eframe default
    let limits   = wgpu::Limits::default();            // == eframe default
    let req_exts: Vec<&'static std::ffi::CStr> = unsafe {
        adapter.as_hal::<wgpu_hal::api::Vulkan, _, _>(|a| {
            a.map(|a| a.required_device_extensions(features))
        })
    }.unwrap_or_default();
    log::info!("wgpu requires exts: {:?}", req_exts);

    // ── 5. Hand the list to C++, which creates the VkDevice
    let mut fig: *mut ffi::VpFigure = std::ptr::null_mut();
    let c_exts: Vec<std::ffi::CString> =
        req_exts.iter().map(|e| e.to_owned()).collect();
    let c_ptrs: Vec<*const i8> = c_exts.iter().map(|e| e.as_ptr()).collect();
    unsafe {
        ffi::vp_create_shared_figure(
            &mut fig,
            c_ptrs.as_ptr(), c_ptrs.len() as i32,
            std::ptr::null(), 0,          // feature blob — phase 0: none
            1280, 720,
        )
    }.check()?;

    let mut dev = ffi::VpSharedDevice::zeroed();
    unsafe { ffi::vp_shared_device(fig, &mut dev) }.check()?;

    // ── 6. Wrap the foreign device
    let ash_dev = unsafe {
        ash::Device::load(ash_inst.fp_v1_0(),
                          vk::Device::from_raw(dev.vk_device))
    };
    let open = unsafe {
        adapter.as_hal::<wgpu_hal::api::Vulkan, _, _>(|a| {
            a.map(|a| a.device_from_raw(
                ash_dev,
                Some(Box::new(|_| {})),       // VP owns vkDestroyDevice
                &req_exts,
                features,
                &limits,
                &wgpu::MemoryHints::Performance,
                dev.family_index,
                dev.queue_index,
            ))
        })
    }.flatten().expect("hal adapter")?;
    let (device, queue) = unsafe {
        adapter.create_device_from_hal::<wgpu_hal::api::Vulkan>(
            open,
            &wgpu::DeviceDescriptor::default(),
        )?
    };

    // ── 7. Trivial proof: submit + poll on the foreign device
    let enc = device.create_command_encoder(&Default::default());
    queue.submit([enc.finish()]);
    device.poll(wgpu::PollType::Wait)?;
    log::info!("wgpu alive on VP's VkDevice ✔");

    // ── 8. Boot eframe on it
    let opts = eframe::NativeOptions {
        wgpu_options: egui_wgpu::WgpuConfiguration {
            wgpu_setup: egui_wgpu::WgpuSetup::Existing(
                egui_wgpu::WgpuSetupExisting {
                    instance, adapter,
                    device: device.clone(), queue: queue.clone(),
                }),
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native("hal-probe", opts,
        Box::new(|_| Ok(Box::new(ProbeApp { _fig: fig }))))?;
    Ok(())
}

struct ProbeApp { _fig: *mut ffi::VpFigure }
impl eframe::App for ProbeApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("wgpu on VolcanoPlot's VkDevice");
        });
    }
    // on_exit: join worker, drop fig, THEN let eframe drop wgpu.
}
```

Known signature drift traps (fix at implementation time):

- `Instance::from_raw`'s argument order and the flags/hints fields
  changed between wgpu 24→30 — compile against `wgpu-hal-30.0.1`
  source open alongside.
- `wgpu::DeviceDescriptor` fields (required_features etc.) moved
  between versions.
- `ash::Instance::fp_v1_0` vs `static_fn` for `Device::load` — use the
  instance-level fp table, NOT `entry`, or function pointers for
  device commands may route wrong.
- `poll(PollType::Wait)` vs older `Maintain::Wait`.

## 20. Appendix E — Phase-1 worker/UI code sketches

### 20.1 Worker render loop delta

```rust
// surface.rs — replaces the vp_render_rgba_view branch when shm on
enum OutFrame {
    Pixels(egui::ColorImage),          // legacy path
    Slot { idx: usize, generation: u32 }, // shm path
}

// in the worker job handler:
fn render_shm(&mut self, fig: &mut VolcanoFigure)
    -> Result<OutFrame>
{
    // Acquire: block (with 500ms timeout) for a Free slot.
    let idx = self.slots.acquire_free_timeout()?;
    self.slots.mark_rendering(idx);

    // uploads + prepare happen inside render_slot, unchanged
    let rendered = unsafe {
        ffi::vp_render_slot(fig.raw, &mut idx_out)
    };
    match rendered {
        0 => { self.slots.mark_ready(idx); Ok(OutFrame::Slot {
                  idx, generation: self.slots.generation() }) }
        _ => { self.slots.mark_free(idx);
               Err(anyhow!("vp_render_slot failed")) }
    }
}
```

### 20.2 UI consume path

```rust
// inside the scope painter / texture registration site:
if let Some(report) = surface.take_report() {
    match report.frame {
        OutFrame::Slot { idx, generation } => {
            // 1. ensure wgpu texture exists for (idx, generation)
            let tex = shared.wrap_slot_image(idx, generation)?;   // §20.3
            // 2. order GPU: wait render_done before egui's submits
            shared.inject_wait(slot.semaphore(idx))?;             // §20.4
            // 3. register with egui (cheap if unchanged)
            tex_manager.set(tex_id_for(idx), tex.view.clone());
            // 4. release back to worker after wgpu's submits retire
            let tx = self.release_tx.clone();
            shared.queue.on_submitted_work_done(move || {
                let _ = tx.send((idx, generation));
            });
            self.displayed = tex_id_for(idx);
        }
        OutFrame::Pixels(img) => { /* existing path */ }
    }
}
```

### 20.3 `wrap_slot_image` (Phase 2; Phase 1 wraps buffer instead)

```rust
pub fn wrap_slot_image(&self, idx: usize, gen: u32)
    -> Result<wgpu::Texture>
{
    // Cache: (idx, gen) → wgpu::Texture; rewrap only on gen change.
    if let Some(t) = self.tex_cache.get(&(idx, gen)) { return Ok(t.clone()); }

    let h = self.slot_handles(idx)?;   // FFI → VpSlotHandles
    let hal_tex = unsafe {
        self.device.as_hal::<wgpu_hal::api::Vulkan, _, _>(|d| {
            d.map(|d| d.texture_from_raw(
                vk::Image::from_raw(h.image),
                &wgpu_hal::TextureDescriptor {
                    label: Some("vp-frame"),
                    size: wgpu::Extent3d {
                        width: h.width, height: h.height, depth_or_array_layers: 1 },
                    mip_level_count: 1, sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm, // match C++
                    usage: wgpu_hal::TextureUses::RESOURCE
                         | wgpu_hal::TextureUses::COPY_DST
                         | wgpu_hal::TextureUses::COLOR_TARGET,
                    memory_flags: wgpu_hal::MemoryFlags::empty(),
                    view_formats: vec![],
                },
                Some(Box::new(|_| {})),    // VP owns image+lifetime
                wgpu_hal::vulkan::TextureMemory::External,
            ))
        })
    }.flatten().expect("hal device");

    let tex = unsafe {
        self.device.create_texture_from_hal::<wgpu_hal::api::Vulkan>(
            hal_tex,
            &wgpu::TextureDescriptor {
                label: Some("vp-frame"),
                size: wgpu::Extent3d {
                    width: h.width, height: h.height,
                    depth_or_array_layers: 1 },
                mip_level_count: 1, sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                     | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
    };
    self.tex_cache.insert((idx, gen), tex.clone());
    Ok(tex)
}
```

### 20.4 `inject_wait` — the one-function synchronization core

```rust
pub fn inject_wait(&self, wait_sem: vk::Semaphore) -> Result<()> {
    unsafe {
        self.device.as_hal::<wgpu_hal::api::Vulkan, _, _>(|d| {
            let d = d.expect("hal device");
            let raw: &ash::Device = d.raw_device();
            let q: vk::Queue      = d.raw_queue();
            let waits   = [wait_sem];
            let stages  = [vk::PipelineStageFlags::FRAGMENT_SHADER
                           | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let submit  = vk::SubmitInfo::default()
                .wait_semaphores(&waits)
                .wait_dst_stage_mask(&stages);
            // Empty cmd list: pure ordering barrier on wgpu's queue.
            // Must run under wgpu's submission lock — wgpu-hal tracks
            // pending submits; racing a raw queue_submit with wgpu's own
            // is UB-adjacent. Hold a mutex that also wraps wgpu submits,
            // or inject from a point where wgpu is known idle
            // (between frames — i.e., before eframe's end-of-paint
            // submission, scheduled via egui callback).
            raw.queue_submit(q, &[submit], vk::Fence::null())
        })
    }?;
    Ok(())
}
```

**Critical subtlety**: `queue_submit` on wgpu's raw queue from outside
wgpu races with wgpu's internal `queue_submit`. Vulkan requires queue
operations on the same `VkQueue` be externally synchronized — a mutex
is mandatory. Options:

- Inject via `egui_wgpu::Callback::prepare`/`paint` — runs inside
  wgpu's submit sequence on the right thread. **Preferred.**
- A global `SUBMIT_LOCK: Mutex<()>` held around BOTH our injection and
  (unfortunately impossible) wgpu's internal submits — can't hold
  wgpu's lock; so callback injection is the only clean route. egui's
  paint callbacks receive `egui_wgpu::CallbackResources` + encoder —
  but raw vkQueueSubmit inside a Callback happens before wgpu
  finalizes its encoder → still ordering-safe since the wait is queued
  before wgpu's submit lands.

### 20.5 C++ `renderFrameInto` sketch

```cpp
// Renderer.cpp — slot-aware submit
int Renderer::renderFrameInto(Figure& fig, FrameSlot& slot) {
    prepare(fig);
    auto cmd = beginFrameScratch();            // existing
    // transition → COLOR_ATTACHMENT_OPTIMAL (if not eGeneral)
    transitionForRender(cmd, slot.image);
    recordAllPlotDraws(fig, cmd);              // existing body
    // transition → SHADER_READ_ONLY_OPTIMAL (if transitions enabled)
    transitionForSample(cmd, slot.image);
    cmd.end();
    vk::SubmitInfo si{};
    si.setCommandBuffers(cmd);
    si.setSignalSemaphores(slot.renderDone);
    graphicsQueue_.submit(si);                 // no fence: display path
    return 0;
}
```

Phase-1 variant appends `vkCmdCopyImageToBuffer(slot.image →
slot.buffer)` before `.end()` — same semaphore covers the copy.

## 21. Appendix F — the two-bring-up-problem, solved

Chicken-and-egg: `required_device_extensions()` needs an adapter,
which needs a physical device; the VkDevice must know the extensions
before `vkCreateDevice`; `device_from_raw` needs the device to exist.
Two FFI entry points resolve it:

```
Rust                                  C++
────                                  ───
vp_probe_device(&probe)          →    create VkInstance (+ wgpu-req'd
                                      instance exts: KHR_surface,
                                      KHR_get_physical_device_properties2,
                                      EXT_debug_utils optional),
                                      vkEnumeratePhysicalDevices → pick
                                      same policy as today → probe
wrap instance, expose_adapter    ←    returns vk_instance, phd, api_ver
required_device_extensions(ftrs)      (no VkDevice yet)
vp_create_shared_figure(exts)    →    vkCreateDevice(exts ∪ VP's,
                                      features2 chain = negotiated),
                                      VMA, backend, slots, figure
device_from_raw, create_device   ←    returns vk_device, family, queues
WgpuSetupExisting → eframe
```

Instance extensions wgpu 30 needs (probe must enable — RADV):
`VK_KHR_surface`, `VK_KHR_get_physical_device_properties2` (mandatory),
plus the surface ext per platform (`VK_KHR_xcb_surface`/`xlib`/`wayland`
— wgpu checks per enabled surface backend; enable xcb+xlib+wayland to
be safe). If `expose_adapter` returns `None`, the reason is almost
always a missing instance extension — the spike logs wgpu's required
set via `Instance::required_extensions()` equivalents and retrying is
a rebuild away.

## 22. Appendix G — memory-format & color-space contract

- Render target format: `VK_FORMAT_R8G8B8A8_UNORM` (current readback
  format) → `wgpu::TextureFormat::Rgba8Unorm`. egui's User textures
  are sampled unmultiplied — matches current `from_rgba_unmultiplied`.
- Keep `Rgba8Unorm` not `…Srgb`: current path treats pixels as unorm;
  switching to srgb would change colors (egui would linearize).
- MSAA resolve: VP already resolves MSAA→single sample into the
  readback target; under shm the resolve target IS the shared image —
  no change to MSAA pipelines.
- Alpha: VP writes opaque alpha=255 — unchanged; egui `tint=WHITE`.
- Origin: Vulkan framebuffer is top-left; egui UV `(0,0)..(1,1)` maps
  correctly only if image orientation matches today's readback
  (readback flips? verify: current code uses the RGBA bytes directly
  in ColorImage top-left — if VP's copyImageToBuffer produced top-left
  origin data, the shared image has the same layout ⇒ `uv` identity).
  If flipped, flip `uv` — one-line fix, caught by the first
  pixel-parity e2e test.

## 23. Appendix H — instrumentation for the before/after table

Extend the existing `VP_DEBUG_PERF` / `LIVEPLOT_DEBUG_FPS` probes:

```
shm.wrap_image   — texture_from_raw + registration (per gen)
shm.inject_wait  — queue_submit wall time
shm.release_lag  — frames between Ready→Free transitions
shm.stall        — worker acquire_free_timeout waits
vp.render_slot   — record+submit (replaces render+wait line)
```

Report under `LIVEPLOT_DEBUG_PERF=1` once/second, same format as the
existing `prepare=… render+wait=…` line so `run_stress.sh` diffing
stays one-command.

Acceptance thresholds (RADV iGPU, §0 workload): worker ≤14 %,
UI ≤10 %, total ≤28 % after Phase 1; total ≤20 % after Phase 2.
If Phase 1 shows <10 % improvement, the copy was not the bottleneck —
re-profile before Phase 2 (the ledger in §16 predicts otherwise).

## 24. Appendix I — what explicitly stays unchanged

- `vp_render_rgba` / `vp_render_rgba_view` — the entire FFI test
  suite, headless PNG dumps (`LIVEPLOT_DEBUG_DUMP`), CI, and the
  fallback path keep working on the CPU readback code path forever.
  The shm path is additive.
- Trace ingest (`vp_trace_slide_data_f64`, ring buffers, envelope
  decimation, tessellation, descriptor retirement) — zero changes.
- egui_plot backend — zero changes.
- Public liveplot API — zero changes. `LIVEPLOT_VOLCANO=1` turns on
  the volcano surface; `VP_SHM=1` (default on Linux once stable)
  selects shm vs readback within it.
- `VolcanoSurface`'s public interface (`request`, `take_report`,
  `resize`) — `OutFrame::Slot` is internal plumbing; the scope paint
  site gains a branch but signatures stay.
