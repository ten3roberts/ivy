# Compiling

The best method of incorporating Ivy into a project is either by [crates.io] or
as a submodule.

In addition to registered crates Ivy depends on additional system level
libraries.

For a successful compilation the following dependencies need to be met:
- Vulkan Development Files
- Windowing libraries (X11/Wayland/WINAPI)
- Vulkan validation layers for debug builds
- glslc

## Linux


For compilation of glfw the following libraries need to be present:
- libxi-dev
- libxcursor-dev
- libxinerama-dev
- libxrandr-dev
- libx11-dev


### Fedora
```sh
sudo dnf install libXi-devel libXcursor-devel libXinerama-devel libXrandr-devel
libX11-devel mesa-vulkan-devel vulkan-validation-layers glslc
```

### Debian
```sh
sudo apt install libxi-dev libxcursor-dev libxinerama-dev libXrandr-devel
libx11-dev libvulkan-dev
```
