# RTX 4070 performance work

Select `-Machine 4070` for the RTX 4070 / Ryzen 7 7800X3D / 32 GB
machine. Automatic selection uses the existing vendor profiles. The 4070
profile keeps the console
selection (Series X by default), output mode, texture mip policy, and the
four-shader wave64 allowlist unchanged.

Changes:

- Eight recording, six under-write, four landing, one mapping and three
  pipeline compilation workers instead of the 16-core 4090 machine's counts.
- A 64 KiB minimum for GPU-resident title buffers, a 1536 MiB buffer budget,
  and a separate 1024 MiB budget for memory windows read by shaders.
- With `VICTORY_CARD_RESERVE_BUDGETS=1`, texture admission reserves explicit
  buffer budgets in full and includes
  unused window budgets. Windows may use that reserved room under the same
  97% limit as buffers. Previously, a larger buffer setting reserved at most
  1 GiB, and windows were refused at 80% occupancy without any reservation.

This trades some texture admission headroom for fewer shader accesses over
PCIe and fewer CPU snapshots. It does not remove rendering passes, change
wave64 semantics, lower loop limits, or enable changed-word-only uploads.
The 4070 profile enables the reservation outside `-Safe`. With the switch unset
or set to `0`, the texture reservation is capped at 1 GiB for buffers and
the window occupancy threshold stays at 80%. `-More` can override individual settings.

## Shader research

- [NVIDIA: Advanced API Performance — Intrinsics](https://developer.nvidia.com/blog/advanced-api-performance-intrinsics/)
  recommends wave operations to reduce shared-memory accesses and barriers.
  That is applicable only when the required communication stays within a
  host wave; it does not make two independent 32-lane waves one 64-lane wave.
- [Microsoft: HLSL Wave Size](https://microsoft.github.io/DirectX-Specs/d3d/HLSL_SM_6_6_WaveSize.html)
  requires the selected size to be supported by the device. The RTX 4070
  reports a minimum and maximum of 32; selecting WaveSize(64) is not a solution.
- [Microsoft: Wave Intrinsics](https://github.com/Microsoft/DirectXShaderCompiler/wiki/Wave-Intrinsics)
  specifies active-lane behavior. Replacing cross-wave exchanges with local
  shuffles can change EXEC-mask behavior and output.
- [AMD: RDNA Performance Guide](https://gpuopen.com/learn/rdna-performance-guide/)
  discusses register pressure, wave operations, and avoiding CPU readbacks.
  Host-side memory placement must also match the discrete GPU memory model.
- [Microsoft: Uploading resources](https://learn.microsoft.com/en-us/windows/win32/direct3d12/uploading-resources)
  recommends default resources for data read repeatedly by the GPU rather
  than repeatedly reading upload memory. This informs the buffer/window
  placement changes here.

Vice-Tay already contains group-uniform barrier checks and wave64 correctness
fixes. They remain intact. The observed baseline spends substantial time on
submission preparation and CPU snapshots, so memory placement is the first
optimization tested; no shader speedup is claimed without shader timings.

## Local validation (2026-11-26)

Nine `shadow::window_tests` passed, including reservation accounting, overlapping
windows, tail coverage, and eviction. The diagnostic application build passed.
The original and patched runs both used Series X, diagnostic builds, info logs,
and disabled DRED/memory-map walks. The user loaded the same gameplay scene
and reported that rendering looked the same with the new profile.

Last eight logged samples with more than 1,000 draws per frame:

| | Original 4090 profile | New 4070 profile |
|---|---:|---:|
| Mean logged frame time | 213.4 ms | 76.6 ms |
| FPS sample range | 2.8–7.8 | 5.9–25.0 |
| Mean logged draws/frame | 1,705 | 1,882 |

These are exploratory gameplay samples, not a locked-camera benchmark. Streaming,
camera position, and pipeline-cache warming can affect the comparison. They do
not establish the individual contribution of each setting or long-session
stability. Later sampled frames had no CPU buffer snapshots. Memory pressure
remains: the patched run reached about 25.6 GB private memory on this 32 GB PC.

Raw logs for this local test are in the sibling checkout's ignored `target`
folder: `D:\Games\cool_fox-archive\target\vice-baseline.err.log` and
`D:\Games\cool_fox-archive\target\vice-4070.err.log`.
