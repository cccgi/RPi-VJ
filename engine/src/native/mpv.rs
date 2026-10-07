//! Small audited dynamic libmpv boundary. All objects stay on the GL thread.
//! ABI: mpv client.h/render.h/render_gl.h. No synchronous commands during playback.
use anyhow::{bail, ensure, Context, Result};
use libloading::Library;
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    marker::PhantomData,
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
struct Property {
    name: *const c_char,
    format: c_int,
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
type Observe = unsafe extern "C" fn(Handle, u64, *const c_char, c_int) -> c_int;
type RenderCreate = unsafe extern "C" fn(*mut Handle, Handle, *mut Param) -> c_int;
type Render = unsafe extern "C" fn(Handle, *mut Param) -> c_int;
type Update = unsafe extern "C" fn(Handle) -> u64;
type Callback =
    unsafe extern "C" fn(Handle, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void);

pub struct Mpv {
    handle: Handle,
    render: Handle,
    command: Command,
    wait: Wait,
    observe: Observe,
    render_create: RenderCreate,
    draw: Render,
    update: Update,
    callback: Callback,
    render_free: Destroy,
    destroy: Destroy,
    _library: Library,
    _thread_bound: PhantomData<Rc<()>>,
    pub restarted: bool,
    pub file_loaded: bool,
    pub hwdec: Option<String>,
}

unsafe extern "C" fn resolve(_: *mut c_void, name: *const c_char) -> *mut c_void {
    // SAFETY: libmpv supplies a terminated symbol name; SDL context is current.
    unsafe { sdl2::sys::SDL_GL_GetProcAddress(name) }
}
unsafe extern "C" fn wake(_: *mut c_void) {
    // Main loop polls at display rate. Callback never locks, allocates or calls mpv.
}
fn check(code: c_int) -> Result<()> {
    ensure!(code >= 0, "libmpv API error {code}");
    Ok(())
}

impl Mpv {
    pub fn new() -> Result<Self> {
        let names: &[&str] = if cfg!(target_os = "windows") {
            &["mpv-2.dll"]
        } else {
            &["libmpv.so.2", "libmpv.so.1"]
        };
        // SAFETY: load only the system-installed libmpv ABI; library outlives symbols.
        unsafe {
            let library = names
                .iter()
                .find_map(|name| Library::new(name).ok())
                .context("libmpv missing; install libmpv-dev/runtime")?;
            let create: Create = *library.get(b"mpv_create\0")?;
            let initialize: Initialize = *library.get(b"mpv_initialize\0")?;
            let option: OptionString = *library.get(b"mpv_set_option_string\0")?;
            // Resolve everything before allocating a handle, so symbol failure cannot leak it.
            let mut this = Self {
                handle: ptr::null_mut(),
                render: ptr::null_mut(),
                command: *library.get(b"mpv_command_async\0")?,
                wait: *library.get(b"mpv_wait_event\0")?,
                observe: *library.get(b"mpv_observe_property\0")?,
                render_create: *library.get(b"mpv_render_context_create\0")?,
                draw: *library.get(b"mpv_render_context_render\0")?,
                update: *library.get(b"mpv_render_context_update\0")?,
                callback: *library.get(b"mpv_render_context_set_update_callback\0")?,
                render_free: *library.get(b"mpv_render_context_free\0")?,
                destroy: *library.get(b"mpv_terminate_destroy\0")?,
                _library: library,
                _thread_bound: PhantomData,
                restarted: false,
                file_loaded: false,
                hwdec: None,
            };
            this.handle = create();
            ensure!(!this.handle.is_null(), "mpv_create failed");
            for (key, value) in [
                ("config", "no"),
                ("vo", "libmpv"),
                ("audio", "no"),
                ("terminal", "yes"),
                ("idle", "yes"),
                ("loop-file", "inf"),
                ("hwdec", "auto-safe"),
                ("osd-level", "0"),
                ("pause", "yes"),
                ("input-default-bindings", "no"),
                ("load-scripts", "no"),
            ] {
                check(option(
                    this.handle,
                    CString::new(key)?.as_ptr(),
                    CString::new(value)?.as_ptr(),
                ))
                .with_context(|| format!("mpv option {key}"))?;
            }
            check(initialize(this.handle))?;
            check((this.observe)(this.handle, 0, c"hwdec-current".as_ptr(), 1))?;
            Ok(this)
        }
    }

    /// Call only with the owning SDL GL context current; keep it current through Drop.
    pub fn attach(&mut self) -> Result<()> {
        ensure!(self.render.is_null(), "render context already attached");
        let mut init = GlInit {
            get_proc_address: resolve,
            context: ptr::null_mut(),
        };
        let mut params = [
            Param {
                kind: 1,
                data: c"opengl".as_ptr() as *mut c_void,
            },
            Param {
                kind: 2,
                data: (&mut init as *mut GlInit).cast(),
            },
            Param {
                kind: 0,
                data: ptr::null_mut(),
            },
        ];
        // SAFETY: ABI structs and strings live through call; GL context is current.
        unsafe {
            check((self.render_create)(
                &mut self.render,
                self.handle,
                params.as_mut_ptr(),
            ))?;
            (self.callback)(self.render, Some(wake), ptr::null_mut());
        }
        Ok(())
    }

    pub fn command(&self, args: &[&str]) -> Result<()> {
        let strings: Vec<CString> = args
            .iter()
            .map(|s| CString::new(*s))
            .collect::<Result<_, _>>()?;
        let mut pointers: Vec<_> = strings.iter().map(|s| s.as_ptr()).collect();
        pointers.push(ptr::null());
        // SAFETY: async API copies arguments before returning; explicitly render-thread safe.
        unsafe { check((self.command)(self.handle, 0, pointers.as_ptr())) }
    }
    pub fn load(&mut self, path: &str) -> Result<()> {
        self.restarted = false;
        self.file_loaded = false;
        self.hwdec = None;
        self.command(&["set", "pause", "yes"])?;
        self.command(&["loadfile", path, "replace"])?;
        // A fresh observation is necessary even if the decoder name did not change.
        unsafe {
            check((self.observe)(self.handle, 0, c"hwdec-current".as_ptr(), 1))?;
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Result<()> {
        // SAFETY: zero-timeout polling is render-thread safe; data consumed before next poll.
        unsafe {
            for _ in 0..64 {
                let event = &*(self.wait)(self.handle, 0.0);
                if event.id == 0 {
                    break;
                }
                if event.error < 0 {
                    bail!("mpv event {} failed: {}", event.id, event.error);
                }
                match event.id {
                    1 => bail!("mpv shut down"),
                    7 if !event.data.is_null() => {
                        let end = &*event.data.cast::<EndFile>();
                        if end.reason == 4 {
                            bail!("mpv failed to load/decode clip: {}", end.error);
                        }
                    }
                    21 => self.restarted = true,
                    8 => self.file_loaded = true,
                    22 if !event.data.is_null() => {
                        let property = &*event.data.cast::<Property>();
                        if property.format == 1 && !property.data.is_null() {
                            let string = *property.data.cast::<*const c_char>();
                            if !string.is_null() {
                                self.hwdec =
                                    Some(CStr::from_ptr(string).to_string_lossy().into_owned());
                            }
                        }
                    }
                    24 => bail!("mpv event queue overflow"),
                    _ => {}
                }
            }
        }
        Ok(())
    }
    pub fn hardware_active(&self) -> bool {
        self.hwdec
            .as_deref()
            .is_some_and(|s| !s.is_empty() && s != "no")
    }
    pub fn render(&self, fbo: u32, width: i32, height: i32, force: bool) -> Result<bool> {
        ensure!(!self.render.is_null(), "render context not attached");
        // SAFETY: current GL context, complete FBO supplied by Rust renderer, local ABI args.
        unsafe {
            if (self.update)(self.render) & 1 == 0 && !force {
                return Ok(false);
            }
            let mut fbo = Fbo {
                id: fbo as i32,
                width,
                height,
                format: 0x8058,
            };
            let mut block = 0_i32;
            let mut params = [
                Param {
                    kind: 3,
                    data: (&mut fbo as *mut Fbo).cast(),
                },
                Param {
                    kind: 12,
                    data: (&mut block as *mut i32).cast(),
                },
                Param {
                    kind: 0,
                    data: ptr::null_mut(),
                },
            ];
            check((self.draw)(self.render, params.as_mut_ptr()))?;
        }
        Ok(true)
    }
}
impl Drop for Mpv {
    fn drop(&mut self) {
        // SAFETY: !Send/!Sync; caller retains current GL context until all Mpv objects drop.
        // Advanced control is deliberately disabled for this two-context baseline.
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
