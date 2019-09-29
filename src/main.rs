extern crate byteorder;
extern crate tempfile;
#[macro_use]
extern crate wayland_client;
extern crate wayland_protocols;
extern crate wayland_window;

use byteorder::{NativeEndian, WriteBytesExt};
//use std::cmp;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use tempfile::tempfile;
use wayland_client::{EnvHandler, Proxy, StateToken};
use wayland_client::protocol::{wl_buffer, wl_compositor, wl_shell, wl_shm, wl_shm_pool, wl_subcompositor,
                               wl_surface};
use wayland_protocols::unstable::xdg_shell::v6::client::zxdg_shell_v6::{self, ZxdgShellV6};
use wayland_window::create_frame;
use try_wayland;


wayland_env!(
    WaylandEnv,
    compositor: wl_compositor::WlCompositor,
    subcompositor: wl_subcompositor::WlSubcompositor,
    shm: wl_shm::WlShm
);

struct Window {
    s: wl_surface::WlSurface,
    tmp: File,
    pool: wl_shm_pool::WlShmPool,
    pool_size: usize,
    buf: wl_buffer::WlBuffer,
    newsize: Option<(i32, i32)>,
    closed: bool,
    refresh: bool,
}

fn window_implementation() -> wayland_window::FrameImplementation<StateToken<Window>> {
    wayland_window::FrameImplementation {
        configure: |evqh, token, config, newsize| {
            if let Some((w, h)) = newsize {
                println!("configure newsize: {:?}", (w, h));
                evqh.state().get_mut(token).newsize = Some((w, h))
            }
            println!("configure metadata: {:?}", config);
            evqh.state().get_mut(token).refresh = true;
        },
        close: |evqh, token| {
            println!("close window");
            evqh.state().get_mut(token).closed = true;
        },
        refresh: |evqh, token| {
            evqh.state().get_mut(token).refresh = true;
        },
    }
}

impl Window {
    fn new(surface: wl_surface::WlSurface, shm: &wl_shm::WlShm) -> Window {
        // create a tempfile to write the contents of the window on
        let mut tmp = tempfile().ok().expect("Unable to create a tempfile.");
        // write the contents to it, lets put everything in dark red
        for _ in 0..16 {
            let _ = tmp.write_u32::<NativeEndian>(0xFF880000);
        }
        let _ = tmp.flush();
        let pool = shm.create_pool(tmp.as_raw_fd(), 64);
        let buffer = pool.create_buffer(0, 4, 4, 16, wl_shm::Format::Argb8888)
            .expect("I didn't destroy the pool!");
        Window {
            s: surface,
            tmp: tmp,
            pool: pool,
            pool_size: 64,
            buf: buffer,
            newsize: Some((500, 450)),
            closed: false,
            refresh: false,
        }
    }
    fn resize(&mut self, width: i32, height: i32) {
        self.tmp.seek(SeekFrom::Start(0)).unwrap();
        for i in 0..(width * height) {
            let x = (i % width) as u32;
            let y = (i / width) as u32;
            let w = width as u32;
            let h = height as u32;
            let r : u8 = 255;
            let g : u8 = 255;
	    let b : u8 = 255;
            self.tmp
		.write_u32::<NativeEndian>((0xFF << 24) + ((r as u32) << 16) + ((g as u32) << 8) + (b as u32)).unwrap();
        }
        self.tmp.flush().unwrap();
        if (width * height * 4) as usize > self.pool_size {
            // the buffer has grown, notify the compositor
            self.pool.resize(width * height * 4);
            self.pool_size = (width * height * 4) as usize;
        }
        self.buf.destroy();
        self.buf = self.pool
            .create_buffer(0, width, height, width * 4, wl_shm::Format::Argb8888)
            .expect("Pool should not be dead!");
        self.s.attach(Some(&self.buf), 0, 0);
        self.s.commit();
    }
    fn buf_text(&mut self, mut str1: &str, w:i32,h:i32,x1:i32,y1:i32){
        let (xsize,ysize) = (w,h);
        let block_size = (str1.len() * 17) as i64;
        let jump = 4 * (xsize as i64 - block_size);
        let start_pos = (y1*500*4 +x1*4)as u64 ;
        let _ = self.tmp.seek(SeekFrom::Start(start_pos));
        {
            let mut writer = &self.tmp;
        //font y size:24
            for scanline in 0..24 {
                for c in str1.bytes() {
                    let char_index: usize = if c < 32 || c > 127 {
                        63 - 32
                    } else {
                        c as usize - 32
                };
                let FONT = try_wayland::get_font();
                let bitmap = FONT[char_index * 24 + scanline];
            //fornt x size 17
                for i in 0..17 {
                    let color = if ((bitmap >> i) & 1) == 0 { 0xffffffff } else { 0xff000000 };
                    let _ = writer.write_u32::<NativeEndian>(color);
                    }
                }
                if jump > 0 {
                    let _ = writer.seek(SeekFrom::Current(jump));
            }
        }

        println!("jump:{:?}, block size:{:?}",jump,block_size);
        let new_buffer = self.pool.create_buffer(
            0,
            xsize as i32,
            ysize as i32,
            4 * xsize as i32,
            wl_shm::Format::Argb8888,
            ).expect("Pool should not be dead!");
        self.s.attach(Some(&new_buffer), 0, 0);
        self.s.commit();
        self.buf = Some(new_buffer).unwrap();
        }
    }
	fn line_buffer(&mut self,x1:i32,y1:i32,x2:i32,y2:i32){
	let gg:i32 = (y2-y1)/(x2-x1);
	let ggg:i32 = y1-gg*x1;
	let width = 500;
	let height = 450;
	let mut r: u8  = 255;
	let mut g : u8 = 0;
	let mut b : u8 = 100;
	self.tmp.seek(SeekFrom::Start((y1*width+x1) as u64)).unwrap();
        for i in 0..(y2*width+x2) {
            let x = (i % width) as i32;
            let y = (i / width) as i32;
            let w = width as u32;
            let h = height as u32;
		if y <5+ x*gg+ggg && y> x*gg+ggg -5 && y<y2-((x-x2)*gg)-5 && x>x1-(y-y1)/gg {
		r = 0;
	      	g = 0;
	        b = 0; 
		}else{
            	r= 255;
             	g = 255;
           	b = 255;
		}
            self.tmp.write_u32::<NativeEndian>((0xff << 24) + ((r as u32) << 16) + ((g as u32) << 8) + (b as u32)).unwrap();
        }
        self.tmp.flush().unwrap();
        self.buf.destroy();
        self.buf = self.pool.create_buffer(0, width, height, width * 4, wl_shm::Format::Argb8888).expect("Pool should not be dead!");
        self.s.attach(Some(&self.buf), 0, 0);
        self.s.commit();}
}

fn main() {
    let (display, mut event_queue) = match wayland_client::default_connect() {
        Ok(ret) => ret,
        Err(e) => panic!("Cannot connect to wayland server: {:?}", e),
    };

    let registry = display.get_registry();
    let env_token = EnvHandler::<WaylandEnv>::init(&mut event_queue, &registry);
    event_queue.sync_roundtrip().unwrap();

    // Use `xdg-shell` if its available. Otherwise, fall back to `wl-shell`.
    let (mut xdg_shell, mut wl_shell) = (None, None);
    {
        let state = event_queue.state();
        let env = state.get(&env_token);
        for &(name, ref interface, version) in env.globals() {
            if interface == ZxdgShellV6::interface_name() {
                xdg_shell = Some(registry.bind::<ZxdgShellV6>(version, name));
                break;
            }
        }

        if xdg_shell.is_none() {
            for &(name, ref interface, version) in env.globals() {
                if interface == wl_shell::WlShell::interface_name() {
                    wl_shell = Some(registry.bind::<wl_shell::WlShell>(version, name));
                    break;
                }
            }
        }
    }

    let shell = match (xdg_shell, wl_shell) {
        (Some(shell), _) => {
            // If using xdg-shell, we'll need to answer the pings.
            let shell_implementation = zxdg_shell_v6::Implementation {
                ping: |_, _, shell, serial| {
                    shell.pong(serial);
                },
            };
            event_queue.register(&shell, shell_implementation, ());
            wayland_window::Shell::Xdg(shell)
        }
        (_, Some(shell)) => wayland_window::Shell::Wl(shell),
        _ => panic!("No available shell"),
    };

    // get the env
    let env = event_queue.state().get(&env_token).clone_inner().unwrap();

    // prepare the Window
    let wl_surface = env.compositor.create_surface();
    let window_token = event_queue
        .state()
        .insert(Window::new(wl_surface.clone().unwrap(), &env.shm));


    // find the seat if any
    let seat = event_queue.state().with_value(&env_token, |_, env| {
        for &(id, ref interface, _) in env.globals() {
            if interface == "wl_seat" {
                return Some(registry.bind(1, id));
            }
        }
        None
    });

    let mut frame = create_frame(
        &mut event_queue,
        window_implementation(),
        window_token.clone(),
        &wl_surface,
        16,
        16,
        &env.compositor,
        &env.subcompositor,
        &env.shm,
        &shell,
        seat,
    ).unwrap();
    let str1 = "hello_world\n".to_string();
    let str2 = "hello_world!".to_string();
    let str3 = "hello_worldrg".to_string();
    let str4 = "hello_worldd".to_string();
    frame.set_title("My example window".into());
    frame.set_decorate(true);
    frame.set_min_size(Some((10,10)));
    frame.refresh();
    loop {
        display.flush().unwrap();
        event_queue.dispatch().unwrap();

        // resize if needed
        let keep_going = event_queue.state().with_value(&window_token, |_, window| {
            if let Some((w, h)) = window.newsize.take() {
                frame.resize(w, h);
                window.resize(w, h);
window.buf_text(&str1,w,h,200,200);
		window.line_buffer(20,20,200,200);
                
                //window.buf_text(&str2,w,h);
                //window.buf_text(&str3,w,h);
                //window.buf_text(&str4,w,h);
		frame.refresh();
            } else if window.refresh {
                frame.refresh();
            }
            window.refresh = false;
            !window.closed
        });

        if !keep_going {
            break;
        }
    }
}
