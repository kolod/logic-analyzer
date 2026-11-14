//! Hot-plug events (spec US7, FR-004, SC-012).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use common::{descriptor, registry_fast, wait_until};
use la_devices::harness::TestProvider;
use la_devices::{DeviceEvent, DeviceId, DeviceProvider, DeviceRegistry, RegistryConfig};

const WAIT: Duration = Duration::from_secs(10);

fn rig(push: bool) -> (DeviceRegistry, TestProvider, Receiver<DeviceEvent>) {
    let registry = registry_fast();
    let provider = TestProvider::new("usb");
    provider.set_push(push);
    // Subscribe first: a subscriber sees the events that happen after it subscribed.
    let events = registry.events();
    registry.add_provider(Arc::new(provider.clone()));
    (registry, provider, events)
}

/// Collects exactly `count` events, failing if they do not arrive in time.
fn collect(events: &Receiver<DeviceEvent>, count: usize) -> Vec<DeviceEvent> {
    (0..count)
        .map(|i| {
            events
                .recv_timeout(WAIT)
                .unwrap_or_else(|_| panic!("event {i} of {count} did not arrive"))
        })
        .collect()
}

/// Asserts that nothing more arrives (no duplicates).
fn assert_quiet(events: &Receiver<DeviceEvent>) {
    std::thread::sleep(Duration::from_millis(150));
    let extra: Vec<DeviceEvent> = events.try_iter().collect();
    assert!(extra.is_empty(), "unexpected extra events: {extra:?}");
}

#[test]
fn a_plug_and_an_unplug_each_produce_exactly_one_event_and_update_the_list() {
    let (registry, provider, events) = rig(true);
    let d = descriptor("usb", "one", 8);
    provider.plug(d.clone());
    assert_eq!(collect(&events, 1), vec![DeviceEvent::Appeared(d.clone())]);
    assert!(wait_until(WAIT, || registry.describe(&d.id).is_some()));
    assert_quiet(&events);

    provider.unplug(&d.id);
    assert_eq!(
        collect(&events, 1),
        vec![DeviceEvent::Disappeared(d.id.clone())]
    );
    assert!(wait_until(WAIT, || registry.describe(&d.id).is_none()));
    assert_quiet(&events);
}

#[test]
fn a_hundred_rapid_connect_disconnect_pairs_are_delivered_exactly_once_and_in_order() {
    let (registry, provider, events) = rig(true);
    let mut expected = Vec::new();
    for i in 0..100 {
        let d = descriptor("usb", &format!("dev{i}"), 8);
        expected.push(DeviceEvent::Appeared(d.clone()));
        expected.push(DeviceEvent::Disappeared(d.id.clone()));
        provider.plug(d.clone());
        provider.unplug(&d.id);
    }
    assert_eq!(
        collect(&events, 200),
        expected,
        "200 events, none lost, in order"
    );
    assert_quiet(&events);
    assert_eq!(registry.list().len(), 1, "only the virtual device is left");
}

#[test]
fn a_provider_that_does_not_push_is_noticed_by_the_periodic_scan() {
    let (registry, provider, events) = rig(false);
    let d = descriptor("usb", "quiet", 8);
    provider.plug(d.clone());
    assert_eq!(collect(&events, 1), vec![DeviceEvent::Appeared(d.clone())]);
    assert!(registry.describe(&d.id).is_some());
    assert_quiet(&events);
    provider.unplug(&d.id);
    assert_eq!(
        collect(&events, 1),
        vec![DeviceEvent::Disappeared(d.id.clone())]
    );
    assert_quiet(&events);
}

#[test]
fn events_that_do_not_change_the_list_are_ignored() {
    let (registry, provider, events) = rig(true);
    let d = descriptor("usb", "known", 8);
    provider.plug(d.clone());
    assert_eq!(collect(&events, 1).len(), 1);
    // An Appeared for a device that is already known, a Disappeared for an unknown id.
    assert!(provider.inject(DeviceEvent::Appeared(d.clone())));
    assert!(provider.inject(DeviceEvent::Disappeared(DeviceId::new("usb", "never-seen"))));
    assert_quiet(&events);
    assert_eq!(registry.list().len(), 2);
}

#[test]
fn the_virtual_device_never_disappears_and_is_never_announced_twice() {
    let (registry, provider, events) = rig(true);
    let virtual_id = DeviceId::new("virtual", "0");
    assert!(provider.inject(DeviceEvent::Disappeared(virtual_id.clone())));
    assert!(provider.inject(DeviceEvent::Appeared(la_devices::virtual_descriptor())));
    assert_quiet(&events);
    assert!(registry.describe(&virtual_id).is_some());
}

#[test]
fn two_providers_pushing_concurrently_lose_nothing_and_keep_their_own_order() {
    let registry = registry_fast();
    let events = registry.events();
    let providers: Vec<TestProvider> = ["alpha", "beta"]
        .iter()
        .map(|n| TestProvider::new(n))
        .collect();
    for p in &providers {
        registry.add_provider(Arc::new(p.clone()));
    }
    let handles: Vec<_> = providers
        .iter()
        .cloned()
        .map(|p| {
            std::thread::spawn(move || {
                let name = p.name().to_owned();
                let mut expected = Vec::new();
                for i in 0..50 {
                    let d = descriptor(&name, &format!("d{i}"), 4);
                    expected.push(DeviceEvent::Appeared(d.clone()));
                    expected.push(DeviceEvent::Disappeared(d.id.clone()));
                    p.plug(d.clone());
                    p.unplug(&d.id);
                }
                (name, expected)
            })
        })
        .collect();
    let expected: Vec<(String, Vec<DeviceEvent>)> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();
    let all = collect(&events, 200);
    assert_quiet(&events);
    for (name, wanted) in expected {
        let got: Vec<DeviceEvent> = all
            .iter()
            .filter(|e| match e {
                DeviceEvent::Appeared(d) => d.id.provider() == name,
                DeviceEvent::Disappeared(id) => id.provider() == name,
            })
            .cloned()
            .collect();
        assert_eq!(got, wanted, "provider {name}");
    }
}

#[test]
fn several_subscribers_each_receive_every_event() {
    let (_registry, provider, first) = rig(true);
    let second = _registry.events();
    let d = descriptor("usb", "shared", 8);
    provider.plug(d.clone());
    assert_eq!(collect(&first, 1), vec![DeviceEvent::Appeared(d.clone())]);
    assert_eq!(collect(&second, 1), vec![DeviceEvent::Appeared(d)]);
}

#[test]
fn dropping_the_registry_stops_the_monitor() {
    let provider = TestProvider::new("usb");
    {
        let registry = DeviceRegistry::new(RegistryConfig {
            scan_interval: Duration::from_millis(5),
            scan_timeout: Duration::from_millis(100),
        });
        registry.add_provider(Arc::new(provider.clone()));
        assert!(wait_until(WAIT, || provider.scan_calls() >= 3));
    }
    std::thread::sleep(Duration::from_millis(100));
    let after_drop = provider.scan_calls();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        provider.scan_calls(),
        after_drop,
        "no scans after the registry is gone"
    );

    // Creating and dropping many registries neither panics nor leaves work behind.
    for _ in 0..100 {
        let registry = registry_fast();
        let _ = registry.events();
        drop(registry);
    }
}
