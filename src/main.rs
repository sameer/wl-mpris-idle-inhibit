use std::thread;
use std::time::Duration;

use mpris::PlaybackStatus;
use mpris::PlayerFinder;
use mpris::{Event as MprisEvent, FindingError, Player};
use wayland_client::{
    protocol::{
        __interfaces::WL_COMPOSITOR_INTERFACE,
        wl_compositor::{self, WlCompositor},
        wl_registry::{self, WlRegistry},
        wl_surface::{self, WlSurface},
    },
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols::wp::idle_inhibit::zv1::client::{
    __interfaces::ZWP_IDLE_INHIBIT_MANAGER_V1_INTERFACE,
    zwp_idle_inhibit_manager_v1::{self, ZwpIdleInhibitManagerV1},
    zwp_idle_inhibitor_v1::{self, ZwpIdleInhibitorV1},
};

/// The typical idle timeout is minutes in length.
/// With that in mind, keeping the sleep duration long here
/// will reduce CPU usage while still achieving the desired effect.
const PLAYER_POLL_SLEEP_DURATION: Duration = Duration::from_secs(5);

/// Error message returned by the playerctld daemon when
/// there is no active player.
const PLAYERCTLD_NO_ACTIVE_PLAYER_MESSAGE: &str = "No player is being controlled by playerctld";

fn main() {
    let conn = Connection::connect_to_env().expect("could not connect to Wayland server");
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    let display = conn.display();

    let _registry = display.get_registry(&qh, ()).unwrap();

    let mut state = State::default();
    event_queue.blocking_dispatch(&mut state).unwrap();
    let mut idle_inhibitor = None;
    loop {
        let player_finder = PlayerFinder::new().expect("could not connect to DBus");
        let active_player_opt =
            find_active_player(&player_finder).expect("error while finding active players");

        if let Some(active_player) = active_player_opt {
            idle_inhibitor = idle_inhibitor.or_else(|| {
                let inhibitor = state
                    .idle_inhibit_manager
                    .as_ref()
                    .expect("idle manager should be present")
                    .create_inhibitor(
                        state
                            .surf
                            .as_ref()
                            .expect("wayland surface should be present"),
                        &qh,
                        (),
                    )
                    .expect("could not inhibit idle");
                conn.roundtrip()
                    .expect("failed to request creating idle inhibitor");
                Some(inhibitor)
            });
            println!("Idle inhibited by {}", active_player.identity());
            // Blocks until new events are received.
            // Guaranteed to (eventually) receive a shutdown event which will break this loop.
            loop {
                let events = active_player
                    .events()
                    .expect("couldn't watch for player events");

                let mut event_iterator = events.map(|event| {
                    event.map(|event| {
                        println!("Received event {:?}", event);
                        matches!(
                            event,
                            MprisEvent::PlayerShutDown | MprisEvent::Stopped | MprisEvent::Paused
                        )
                    })
                });

                let should_allow_idle = event_iterator
                    .find(|res| matches!(res, Ok(true) | Err(_)))
                    .unwrap_or_else(|| {
                        println!("No event ending playback returned, allowing idle");
                        Ok(true)
                    })
                    .unwrap_or_else(|err| {
                        println!("Error while watching player events, allowing idle: {}", err);
                        true
                    });

                if should_allow_idle {
                    break;
                }
            }
        } else if let Some(i) = idle_inhibitor.as_ref() {
            i.destroy();
            idle_inhibitor = None;
            conn.roundtrip()
                .expect("failed to request destruction of idle inhibitor");
            println!("Idle allowed");
        }
        thread::sleep(PLAYER_POLL_SLEEP_DURATION)
    }
}

/// Returns the first active player that is found.
///
/// Returns [Ok(None)] when there are no active players
/// and the playerctld daemon returns a D-Bus error.
fn find_active_player(player_finder: &PlayerFinder) -> Result<Option<Player>, FindingError> {
    let res = player_finder.find_all().map(|players| {
        players.into_iter().find(|p| match p.get_playback_status() {
            Ok(PlaybackStatus::Playing) => true,
            Ok(_) => false,
            Err(e) => {
                println!("Could not get playback status for {} {}", p.identity(), e);
                false
            }
        })
    });
    match res {
        Err(FindingError::DBusError(mpris::DBusError::TransportError(ref err)))
            if err.message() == Some(PLAYERCTLD_NO_ACTIVE_PLAYER_MESSAGE) =>
        {
            Ok(None)
        }
        other => other,
    }
}

#[derive(Default)]
struct State {
    compositor: Option<WlCompositor>,
    surf: Option<WlSurface>,
    idle_inhibit_manager: Option<ZwpIdleInhibitManagerV1>,
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        &mut self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == WL_COMPOSITOR_INTERFACE.name => {
                let compositor = registry
                    .bind::<WlCompositor, _, _>(name, version, qh, ())
                    .unwrap();
                self.surf = Some(compositor.create_surface(qh, ()).unwrap());
                self.compositor = Some(compositor);
                eprintln!("[{}] {} (v{})", name, interface, version);
            }
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == ZWP_IDLE_INHIBIT_MANAGER_V1_INTERFACE.name => {
                let idle_inhibit_manager = registry
                    .bind::<ZwpIdleInhibitManagerV1, _, _>(name, version, qh, ())
                    .unwrap();
                self.idle_inhibit_manager = Some(idle_inhibit_manager);
                eprintln!("[{}] {} (v{})", name, interface, version);
            }
            // Don't care
            _ => {}
        }
    }
}

impl Dispatch<WlCompositor, ()> for State {
    fn event(
        &mut self,
        _: &WlCompositor,
        _: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSurface, ()> for State {
    fn event(
        &mut self,
        _: &WlSurface,
        _: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpIdleInhibitManagerV1, ()> for State {
    fn event(
        &mut self,
        _: &ZwpIdleInhibitManagerV1,
        _: zwp_idle_inhibit_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpIdleInhibitorV1, ()> for State {
    fn event(
        &mut self,
        _: &ZwpIdleInhibitorV1,
        _: zwp_idle_inhibitor_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
