# Bundled slang shaders

Taken from https://github.com/libretro/slang-shaders (commit 2345ee9).
Every file below carries a license header compatible with GPL-2.0-or-later;
each file's own header is authoritative. Presets that depend on a GPL-3.0-only
file, or on unlabelled files or images of unclear origin, were left out. The
exceptions are crt-royale (TroggleMonkey) and crt-guest-advanced (guest(r)): both
projects are GPL-2.0-or-later, and their few unlabelled helper files, mask
textures and LUTs are taken under the project license (marked below).
`tools/slang_vet.py` and `tools/slang_pick.py` checked and copied them.

The image `dithering/shaders/blue-noise/LDR_RGB1_0.png` is from Christoph Peters'
free blue noise textures (momentsingraphics.de), released under CC0.

| File | License |
|---|---|
| `anti-aliasing/shaders/advanced-aa.slang` | GPL-2.0-or-later |
| `blurs/shaders/gaussian_blur_filtering/gaussian.slang` | GPL-2.0-or-later |
| `cel/shaders/advcartoon.slang` | GPL (no version stated, so any version) |
| `crt/shaders/CreativeForce/crt-CreativeForce-Arcade.slang` | GPL-2.0-or-later |
| `crt/shaders/CreativeForce/crt-CreativeForce-SharpSmooth.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-aperture.slang` | GPL (no version stated, so any version) |
| `crt/shaders/crt-blurPi.slang` | MIT |
| `crt/shaders/crt-caligari.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-consumer.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-easymode.slang` | GPL (no version stated, so any version) |
| `crt/shaders/crt-gdv-mini-ultra.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-gdv-mini.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-geom.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-lottes-fast.slang` | Public domain |
| `crt/shaders/crt-lottes.slang` | Public domain |
| `crt/shaders/crt-nobody.slang` | MIT |
| `crt/shaders/crt-pi.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-simple.slang` | GPL-2.0-or-later |
| `crt/shaders/gizmo-crt.slang` | GPL-2.0-or-later |
| `crt/shaders/gizmo-slotmask-crt.slang` | GPL-2.0-or-later |
| `crt/shaders/hyllian/crt-hyllian-3d.slang` | MIT |
| `crt/shaders/hyllian/crt-hyllian-fast.slang` | MIT |
| `crt/shaders/monoCRT.slang` | Public domain |
| `crt/shaders/newpixie-mini/newpixie-mini.slang` | MIT |
| `crt/shaders/zfast_crt/zfast_crt_composite.slang` | GPL-2.0-or-later |
| `crt/shaders/zfast_crt/zfast_crt_curvature.slang` | GPL-2.0-or-later |
| `crt/shaders/zfast_crt/zfast_crt_geo.slang` | GPL-2.0-or-later |
| `crt/shaders/zfast_crt/zfast_crt_geo_svideo.slang` | GPL-2.0-or-later |
| `deblur/shaders/deblur-luma.slang` | GPL-2.0-or-later |
| `deblur/shaders/deblur.slang` | GPL-2.0-or-later |
| `denoisers/shaders/fast-bilateral.slang` | MIT |
| `dithering/shaders/bayer_4x4.slang` | GPL-2.0-or-later |
| `dithering/shaders/blue_noise.slang` | GPL-2.0-or-later |
| `edge-smoothing/fsr/shaders/ffx_a.h` | MIT |
| `edge-smoothing/fsr/shaders/ffx_fsr1.h` | MIT |
| `edge-smoothing/fsr/shaders/fsr-pass0.slang` | Public domain |
| `edge-smoothing/fsr/shaders/fsr-pass1.slang` | Public domain |
| `edge-smoothing/nis/shaders/nis-coef.slang` | MIT |
| `edge-smoothing/nis/shaders/nis-luma.slang` | MIT |
| `edge-smoothing/nis/shaders/nis-scaler.slang` | MIT |
| `gpu/shaders/3dfx/3dfx_pass_0.slang` | GPL-2.0-or-later |
| `gpu/shaders/3dfx/3dfx_pass_1.slang` | GPL-2.0-or-later |
| `gpu/shaders/3dfx/3dfx_pass_2.slang` | GPL-2.0-or-later |
| `gpu/shaders/powervr2.slang` | GPL-2.0-or-later |
| `include/colorspace-tools.h` | GPL (no version stated, so any version) |
| `include/subpixel_masks.h` | Public domain |
| `interpolation/shaders/EWA-Cubics.slang` | GPL-2.0-or-later |
| `interpolation/shaders/bicubic/bicubic-x.slang` | MIT |
| `interpolation/shaders/bicubic/bicubic-y.slang` | MIT |
| `misc/shaders/color-mangler.slang` | Public domain |
| `misc/shaders/deband.slang` | GPL-2.0-or-later |
| `misc/shaders/grade-no-LUT.slang` | GPL-2.0-or-later |
| `misc/shaders/image-adjustment.slang` | Public domain |
| `motionblur/shaders/motionblur-simple.slang` | GPL-2.0-or-later |
| `ntsc/shaders/ntsc-simple/tiny_ntsc.slang` | GPL-2.0-or-later |
| `pal/shaders/pal-singlepass.slang` | BSD |
| `scanlines/shaders/res-independent-scanlines.slang` | Public domain |
| `scanlines/shaders/scanline-fract.slang` | Public domain |
| `scanlines/shaders/scanlines-rere.slang` | Public domain |
| `scanlines/shaders/scanlines-sine-abs.slang` | Public domain |
| `sharpen/shaders/adaptive-sharpen-pass1.slang` | BSD |
| `sharpen/shaders/adaptive-sharpen-pass2.slang` | BSD |
| `sharpen/shaders/adaptive-sharpen.slang` | BSD |
| `sharpen/shaders/cheap-sharpen.slang` | MIT |
| `sharpen/shaders/rcas.slang` | MIT |
| `blurs/shaders/royale/blur9fast-horizontal.slang` | MIT |
| `blurs/shaders/royale/blur9fast-vertical.slang` | MIT |
| `blurs/shaders/royale/vertex-shader-blur-fast-horizontal.h` | MIT |
| `blurs/shaders/royale/vertex-shader-blur-fast-vertical.h` | MIT |
| `crt/shaders/crt-royale/TileableLinearApertureGrille15Wide8And5d5Spacing.png` | image; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/TileableLinearApertureGrille15Wide8And5d5SpacingResizeTo64.png` | image; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/TileableLinearApertureGrille15Wide8And5d5SpacingResizeTo64BGR.png` | image; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/TileableLinearShadowMaskEDP.png` | image; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/TileableLinearShadowMaskEDPResizeTo64.png` | image; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/TileableLinearSlotMaskTall15Wide9And4d5Horizontal9d14VerticalSpacing.png` | image; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/TileableLinearSlotMaskTall15Wide9And4d5Horizontal9d14VerticalSpacingResizeTo64.png` | image; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/TileableLinearSlotMaskTall15Wide9And4d5Horizontal9d14VerticalSpacingResizeTo64BGRshifted.png` | image; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/src-fast/bind-shader-params.h` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src-fast/crt-royale-bloom-horizontal-reconstitute.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src-fast/crt-royale-bloom-vertical.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src-fast/crt-royale-brightpass.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src-fast/crt-royale-first-pass-linearize-crt-gamma-bob-fields.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src-fast/crt-royale-mask-resize-horizontal.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src-fast/crt-royale-mask-resize-vertical.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src-fast/crt-royale-scanlines-horizontal-apply-mask.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src-fast/crt-royale-scanlines-vertical-interlacing.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src-fast/derived-settings-and-constants.h` | no header; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/src-fast/phosphor-mask-resizing.h` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src-fast/scanline-functions.h` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src-fast/special-functions.h` | MIT |
| `crt/shaders/crt-royale/src-fast/user-cgp-constants.h` | no header; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/src-fast/user-settings.h` | no header; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/src/bind-shader-params.h` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/bloom-functions.h` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/crt-royale-bloom-approx-fake-bloom.slang` | no header; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/src/crt-royale-bloom-approx.h` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/crt-royale-bloom-approx.slang` | no header; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/src/crt-royale-bloom-horizontal-reconstitute.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/crt-royale-bloom-vertical.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/crt-royale-brightpass.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/crt-royale-first-pass-linearize-crt-gamma-bob-fields.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/crt-royale-geometry-aa-last-pass.h` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/crt-royale-geometry-aa-last-pass.slang` | no header; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/src/crt-royale-mask-resize-horizontal.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/crt-royale-mask-resize-vertical.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/crt-royale-scanlines-horizontal-apply-mask-fake-bloom.slang` | no header; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/src/crt-royale-scanlines-horizontal-apply-mask.h` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/crt-royale-scanlines-horizontal-apply-mask.slang` | no header; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/src/crt-royale-scanlines-vertical-interlacing.slang` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/derived-settings-and-constants.h` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/geometry-functions.h` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/phosphor-mask-resizing.h` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/scanline-functions.h` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/tex2Dantialias.h` | GPL-2.0-or-later |
| `crt/shaders/crt-royale/src/user-cgp-constants.h` | no header; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/crt-royale/user-settings.h` | no header; GPL-2.0-or-later as part of the crt-royale project |
| `crt/shaders/guest/advanced/afterglow0.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/avg-lum-ntsc.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/avg-lum.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/bloom_horizontal.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/bloom_vertical.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/crt-guest-advanced-ntsc-pass1.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/crt-guest-advanced-ntsc-pass2.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/crt-guest-advanced.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/custom-fast-sharpen.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/deconvergence-ntsc.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/deconvergence.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/gaussian_horizontal.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/gaussian_vertical.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/interlace-ntsc.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/linearize-ntsc-new.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/linearize.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/lut/inv-trinitron-lut.png` | image; GPL-2.0-or-later as part of the guest(r) crt-guest-advanced project |
| `crt/shaders/guest/advanced/lut/nec-lut.png` | image; GPL-2.0-or-later as part of the guest(r) crt-guest-advanced project |
| `crt/shaders/guest/advanced/lut/ntsc-lut.png` | image; GPL-2.0-or-later as part of the guest(r) crt-guest-advanced project |
| `crt/shaders/guest/advanced/lut/trinitron-lut.png` | image; GPL-2.0-or-later as part of the guest(r) crt-guest-advanced project |
| `crt/shaders/guest/advanced/ntsc/ntsc-pass1.slang` | no header; GPL-2.0-or-later as part of the guest(r) crt-guest-advanced project |
| `crt/shaders/guest/advanced/ntsc/ntsc-pass2.slang` | no header; GPL-2.0-or-later as part of the guest(r) crt-guest-advanced project |
| `crt/shaders/guest/advanced/ntsc/ntsc-pass3.slang` | no header; GPL-2.0-or-later as part of the guest(r) crt-guest-advanced project |
| `crt/shaders/guest/advanced/pre-shaders-afterglow.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/advanced/stock.slang` | no header; GPL-2.0-or-later as part of the guest(r) crt-guest-advanced project |
| `crt/shaders/guest/fast/bloom_horizontal.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/fast/bloom_vertical.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/fast/crt-guest-advanced-pass1.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/fast/crt-guest-advanced-pass1f.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/fast/crt-guest-advanced-pass2.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/fast/crt-guest-advanced-pass2f.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/fast/deconvergence-f.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/fast/linearize.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/fast/linearizef.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/fast/perf-pass.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/fast/pre-shaders.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/fast/pre-shadersf.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/fast/stock.slang` | no header; GPL-2.0-or-later as part of the guest(r) crt-guest-advanced project |
| `crt/shaders/guest/hd/afterglow0.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/hd/bloom_horizontal.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/hd/bloom_vertical.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/hd/crt-guest-advanced-hd-pass1.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/hd/crt-guest-advanced-hd-pass2.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/hd/deconvergence-hd.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/hd/gaussian_horizontal.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/hd/gaussian_vertical.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/hd/linearize-hd.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/hd/pre-shaders-afterglow.slang` | GPL-2.0-or-later |
| `crt/shaders/guest/hd/stock.slang` | no header; GPL-2.0-or-later as part of the guest(r) crt-guest-advanced project |
| `include/blur-functions.h` | MIT |
| `include/compat_macros.inc` | no header; macro definitions only |
| `include/gamma-management.h` | MIT |
| `include/quad-pixel-communication.h` | MIT |
| `include/special-functions.h` | MIT |
