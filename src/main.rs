use std::thread;
use std::time::Duration;

use mpris::Event as MprisEvent;
use mpris::PlaybackStatus;
use mpris::PlayerFinder;
use wayland_client::{
    protocol::{
        __interfaces::WL_COMPOSITOR_INTERFACE,
        wl_compositor::{self, WlCompositor},
        wl_registry::{self, WlRegistry},
        wl_surface::{self, WlSurface},
    },
    Connection, ConnectionHandle, Dispatch, QueueHandle,
};
use wayland_protocols::unstable::idle_inhibit::v1::client::{
    __interfaces::ZWP_IDLE_INHIBIT_MANAGER_V1_INTERFACE,
    zwp_idle_inhibit_manager_v1::{self, ZwpIdleInhibitManagerV1},
    zwp_idle_inhibitor_v1::{self, ZwpIdleInhibitorV1},
};

/// The typical idle timeout is minutes in length.
/// With that in mind, keeping the sleep duration long here
/// will reduce CPU usage while still achieving the desired effect.
const PLAYER_POLL_SLEEP_DURATION: Duration = Duration::from_secs(5);

fn main() {
    let player_finder = PlayerFinder::new().expect("could not connect to DBus");

    let conn = Connection::connect_to_env().expect("could not connect to Wayland server");
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    let display = conn.handle().display();

    let _registry = display.get_registry(&mut conn.handle(), &qh, ()).unwrap();

    let mut state = State::default();
    event_queue.blocking_dispatch(&mut state).unwrap();
    let mut idle_inhibitor = None;
    loop {
        let active_player_opt = player_finder
            .find_all()
            .unwrap()
            .drain(..)
            .find(|p| p.get_playback_status().unwrap() == PlaybackStatus::Playing);
        if let Some(active_player) = active_player_opt {
            idle_inhibitor = idle_inhibitor.or_else(|| {
                Some(
                    state
                        .idle_inhibit_manager
                        .as_ref()
                        .unwrap()
                        .create_inhibitor(&mut conn.handle(), state.surf.as_ref().unwrap(), &qh, ())
                        .unwrap(),
                )
            });
            println!("Idle inhibited by {}", active_player.identity());
            // Blocks until new events are received.
            // Guaranteed to (eventually) receive a shutdown event which will break this loop.
            while !active_player.events().unwrap().any(|event| {
                matches!(
                    event.unwrap(),
                    MprisEvent::PlayerShutDown | MprisEvent::Stopped | MprisEvent::Paused
                )
            }) {}
            // Allows immediate decision on whether to destroy idle inhibitor
            continue;
        } else if let Some(idle_inhibitor) = idle_inhibitor.as_ref() {
            idle_inhibitor.destroy(&mut conn.handle());
            println!("Idle allowed");
        }
        thread::sleep(PLAYER_POLL_SLEEP_DURATION)
    }
}

#[derive(Default)]
struct State {
    compositor: Option<WlCompositor>,
    surf: Option<WlSurface>,
    idle_inhibit_manager: Option<ZwpIdleInhibitManagerV1>,
}

impl Dispatch<WlRegistry> for State {
    type UserData = ();

    fn event(
        &mut self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &Self::UserData,
        conn: &mut ConnectionHandle,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == ZWP_IDLE_INHIBIT_MANAGER_V1_INTERFACE.name {
                let idle_inhibit_manager = registry
                    .bind::<ZwpIdleInhibitManagerV1, _>(conn, name, version, qh, ())
                    .unwrap();
                self.idle_inhibit_manager = Some(idle_inhibit_manager);
                eprintln!("[{}] {} (v{})", name, interface, version);
            } else if interface == WL_COMPOSITOR_INTERFACE.name {
                let compositor = registry
                    .bind::<WlCompositor, _>(conn, name, version, qh, ())
                    .unwrap();
                let surf = compositor.create_surface(conn, qh, ()).unwrap();
                self.surf = Some(surf);
                self.compositor = Some(compositor);
                eprintln!("[{}] {} (v{})", name, interface, version);
            }
        }
    }
}

impl Dispatch<WlCompositor> for State {
    type UserData = ();

    fn event(
        &mut self,
        _: &WlCompositor,
        _: wl_compositor::Event,
        _: &Self::UserData,
        _: &mut ConnectionHandle,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSurface> for State {
    type UserData = ();

    fn event(
        &mut self,
        _: &WlSurface,
        _: wl_surface::Event,
        _: &Self::UserData,
        _: &mut ConnectionHandle,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpIdleInhibitManagerV1> for State {
    type UserData = ();

    fn event(
        &mut self,
        _: &ZwpIdleInhibitManagerV1,
        _: zwp_idle_inhibit_manager_v1::Event,
        _: &Self::UserData,
        _: &mut ConnectionHandle,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpIdleInhibitorV1> for State {
    type UserData = ();

    fn event(
        &mut self,
        _: &ZwpIdleInhibitorV1,
        _: zwp_idle_inhibitor_v1::Event,
        _: &Self::UserData,
        _: &mut ConnectionHandle,
        _: &QueueHandle<Self>,
    ) {
    }
}
