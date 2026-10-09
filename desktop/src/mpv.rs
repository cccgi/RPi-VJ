//! Video clips through libmpv's render API, drawn into one of the app's render targets.
//! The same boundary as the Pi engine's `native/mpv.rs` (client.h, render.h,
//! render_gl.h), with the loader and library names of a desktop. All of it stays on the
//! GL thread.
use crate::gfx::Target;
use anyhow::{bail, ensure, Context, Result};
use glow::HasContext;
use libloading::Library;
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    marker::PhantomData,
    path::{Path, PathBuf},
    ptr,
    rc::Rc,
};

#[repr(C)]
struct Param {
    kind: c_int,
    data: *mut c_void,
}
#[repr(C)]
struct GlInit {
    get_proc_address: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
    context: *mut c_void,
}
#[repr(C)]
struct Fbo {
    id: c_int,
    width: c_int,
    height: c_int,
    format: c_int,
}
#[repr(C)]
struct Event {
    id: c_int,
    error: c_int,
    userdata: u64,
    data: *mut c_void,
}
#[repr(C)]
struct EndFile {
    reason: c_int,
    error: c_int,
}
type Handle = *mut c_void;
type Create = unsafe extern "C" fn() -> Handle;
type Initialize = unsafe extern "C" fn(Handle) -> c_int;
type OptionString = unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int;
type Destroy = unsafe extern "C" fn(Handle);
type Command = unsafe extern "C" fn(Handle, u64, *const *const c_char) -> c_int;
type Wait = unsafe extern "C" fn(Handle, f64) -> *const Event;
type RenderCreate = unsafe extern "C" fn(*mut Handle, Handle, *mut Param) -> c_int;
type Render = unsafe extern "C" fn(Handle, *mut Param) -> c_int;
type Update = unsafe extern "C" fn(Handle) -> u64;

/// Looks a GL function up for the player: first among the driver's functions, then
/// among the few old ones that `opengl32.dll` itself exports.
#[cfg(windows)]
unsafe extern "C" fn resolve(_: *mut c_void, name: *const c_char) -> *mut c_void {
    type Lookup = unsafe extern "system" fn(*const c_char) -> *mut c_void;
    static OPENGL: std::sync::OnceLock<Option<Library>> = std::sync::OnceLock::new();
    // SAFETY: the library stays loaded for the life of the program; libmpv passes a
    // terminated name, and the GL context is current whenever it asks.
    unsafe {
        let Some(library) = OPENGL.get_or_init(|| Library::new("opengl32.dll").ok()) else { return ptr::null_mut() };
        if let Ok(lookup) = library.get::<Lookup>(b"wglGetProcAddress\0") {
            let found = lookup(name);
            // The driver answers small numbers or -1 for names it does not have.
            if (found as usize) > 3 && (found as isize) != -1 {
                return found;
            }
        }
        library.get::<*mut c_void>(CStr::from_ptr(name).to_bytes_with_nul()).map_or(ptr::null_mut(), |symbol| *symbol)
    }
}
#[cfg(not(windows))]
unsafe extern "C" fn resolve(_: *mut c_void, _: *const c_char) -> *mut c_void {
    ptr::null_mut()
}
fn check(code: c_int) -> Result<()> {
    ensure!(code >= 0, "libmpv API error {code}");
    Ok(())
}
/// Where the player library may sit: beside the program, in the working directory, or
/// in the repository's `target/desktop/mpv`.
fn candidates() -> Vec<PathBuf> {
    let mut folders = Vec::new();
    if let Some(folder) = std::env::var_os("RPI_VJ_MPV") {
        folders.push(PathBuf::from(folder));
    }
    if let Some(folder) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_owned)) {
        folders.push(folder);
    }
    folders.push(crate::library::root().join("target/desktop/mpv"));
    let names: &[&str] =
        if cfg!(target_os = "windows") { &["libmpv-2.dll", "mpv-2.dll"] } else { &["libmpv.so.2", "libmpv.so.1"] };
    let mut paths: Vec<PathBuf> =
        folders.iter().flat_map(|folder| names.iter().map(move |name| folder.join(name))).collect();
    // Last, by bare name: the system's own search path.
    paths.extend(names.iter().map(PathBuf::from));
    paths
}
pub struct Player {
    handle: Handle,
    render: Handle,
    command: Command,
    wait: Wait,
    draw: Render,
    update: Update,
    render_free: Destroy,
    destroy: Destroy,
    _library: Library,
    _thread_bound: PhantomData<Rc<()>>,
}
impl Player {
    /// Call with the app's GL context current; it must stay current until the drop.
    pub fn new() -> Result<Self> {
        // SAFETY: the library outlives every symbol taken from it; ABI structs and
        // strings live through the calls they are passed to.
        unsafe {
            let library = candidates()
                .iter()
                .find_map(|path| Library::new(path).ok())
                .context("the mpv player library (libmpv-2.dll) was not found; video clips are off")?;
            let create: Create = *library.get(b"mpv_create\0")?;
            let initialize: Initialize = *library.get(b"mpv_initialize\0")?;
            let option: OptionString = *library.get(b"mpv_set_option_string\0")?;
            let render_create: RenderCreate = *library.get(b"mpv_render_context_create\0")?;
            // Everything is resolved before a handle exists, so a missing symbol leaks nothing.
            let mut this = Self {
                handle: ptr::null_mut(),
                render: ptr::null_mut(),
                command: *library.get(b"mpv_command_async\0")?,
                wait: *library.get(b"mpv_wait_event\0")?,
                draw: *library.get(b"mpv_render_context_render\0")?,
                update: *library.get(b"mpv_render_context_update\0")?,
                render_free: *library.get(b"mpv_render_context_free\0")?,
                destroy: *library.get(b"mpv_terminate_destroy\0")?,
                _library: library,
                _thread_bound: PhantomData,
            };
            this.handle = create();
            ensure!(!this.handle.is_null(), "mpv_create failed");
            for (key, value) in [
                ("config", "no"),
                ("vo", "libmpv"),
                ("audio", "no"),
                ("terminal", "no"),
                ("idle", "yes"),
                ("loop-file", "inf"),
                // Decode on the graphics card where that is safe, and hand frames over
                // as ordinary memory: no dependence on how the window was made.
                ("hwdec", "auto-copy-safe"),
                ("osd-level", "0"),
                ("input-default-bindings", "no"),
                ("load-scripts", "no"),
            ] {
                check(option(this.handle, CString::new(key)?.as_ptr(), CString::new(value)?.as_ptr()))
                    .with_context(|| format!("mpv option {key}"))?;
            }
            check(initialize(this.handle))?;
            let mut init = GlInit { get_proc_address: resolve, context: ptr::null_mut() };
            let mut params = [
                Param { kind: 1, data: c"opengl".as_ptr() as *mut c_void },
                Param { kind: 2, data: (&mut init as *mut GlInit).cast() },
                Param { kind: 0, data: ptr::null_mut() },
            ];
            check(render_create(&mut this.render, this.handle, params.as_mut_ptr())).context("mpv render context")?;
            Ok(this)
        }
    }
    fn command(&self, args: &[&str]) -> Result<()> {
        let strings: Vec<CString> = args.iter().map(|s| CString::new(*s)).collect::<Result<_, _>>()?;
        let mut pointers: Vec<_> = strings.iter().map(|s| s.as_ptr()).collect();
        pointers.push(ptr::null());
        // SAFETY: the async call copies its arguments before it returns.
        unsafe { check((self.command)(self.handle, 0, pointers.as_ptr())) }
    }
    /// Play `clip` from its start, looping.
    pub fn load(&mut self, clip: &Path) -> Result<()> {
        let path = clip.to_str().context("the clip's path is not plain text")?;
        self.command(&["loadfile", path, "replace"])?;
        self.command(&["set", "pause", "no"])
    }
    pub fn stop(&mut self) -> Result<()> {
        self.command(&["stop"])
    }
    /// Take what the player has to say; an error means the clip could not be played.
    pub fn poll(&mut self) -> Result<()> {
        // SAFETY: zero-timeout polling on the owning thread; each event is read before
        // the next call replaces it.
        unsafe {
            for _ in 0..64 {
                let event = &*(self.wait)(self.handle, 0.0);
                match event.id {
                    0 => break,
                    1 => bail!("the player shut down"),
                    7 if !event.data.is_null() => {
                        let end = &*event.data.cast::<EndFile>();
                        if end.reason == 4 {
                            bail!("the clip could not be loaded or decoded (mpv error {})", end.error);
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
    /// Draw the clip's current frame into `target` if there is a new one; the target
    /// keeps the last frame otherwise.
    pub fn render(&self, gl: &glow::Context, target: &Target) -> Result<()> {
        // SAFETY: the context is current, the target is a complete framebuffer of this
        // context, and the ABI arguments live through the call.
        unsafe {
            if (self.update)(self.render) & 1 == 0 {
                return Ok(());
            }
            let framebuffer = target.fbo.0.get();
            let mut fbo = Fbo { id: framebuffer as i32, width: target.size.0 as i32, height: target.size.1 as i32, format: 0x8058 };
            // Flipped, the target holds the picture the right way up in GL terms, like
            // the shader layer it is mixed with.
            let (mut flip, mut block) = (1_i32, 0_i32);
            let mut params = [
                Param { kind: 3, data: (&mut fbo as *mut Fbo).cast() },
                Param { kind: 4, data: (&mut flip as *mut i32).cast() },
                Param { kind: 12, data: (&mut block as *mut i32).cast() },
                Param { kind: 0, data: ptr::null_mut() },
            ];
            let drawn = check((self.draw)(self.render, params.as_mut_ptr()));
            // The player leaves its own program and bindings behind.
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.use_program(None);
            gl.bind_vertex_array(None);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, None);
            drawn
        }
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        // SAFETY: not Send or Sync, so this is the GL thread with its context current.
        unsafe {
            if !self.render.is_null() {
                (self.render_free)(self.render);
            }
            if !self.handle.is_null() {
                (self.destroy)(self.handle);
            }
        }
    }
}
