{ pkgs }:

pkgs.mkShell rec {
  nativeBuildInputs = with pkgs; [
    rustc
    cargo
    rustfmt
    clippy
    rust-analyzer
    pkg-config
  ];

  buildInputs = with pkgs; [
    expat
    fontconfig
    freetype
    libGL
    vulkan-loader
    wayland
    libxkbcommon
    libx11
    libxcursor
    libxi
    libxrandr
  ];

  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;

  # wgpu probes every backend at start-up, and NVIDIA's EGL library can refuse the
  # display there, which panics inside khronos-egl. Vulkan is the backend in use anyway.
  WGPU_BACKEND = "vulkan";
}
