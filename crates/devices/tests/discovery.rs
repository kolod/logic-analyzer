//! Discovery and identity (spec US1, FR-001..FR-003, SC-003).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{descriptor, registry_fast, wait_until};
use la_devices::harness::TestProvider;
use la_devices::{DeviceId, DeviceRegistry, SampleRates, TriggerKind, virtual_descriptor};

const WAIT: Duration = Duration::from_secs(5);

fn ids(registry: &DeviceRegistry) -> Vec<String> {
    registry.list().iter().map(|d| d.id.to_string()).collect()
}

#[test]
fn with_no_hardware_the_list_is_exactly_the_virtual_device() {
    let registry = registry_fast();
    let list = registry.list();
    assert_eq!(list.len(), 1);
    let d = &list[0];
    assert_eq!(d.id.as_str(), "virtual:0");
    assert!(d.simulated);
    assert_eq!(d.name, "Virtual device");
    assert_eq!(d.capabilities.channels, 32);
    assert_eq!(
        d.capabilities.sample_rates,
        SampleRates::Range {
            min: 1,
            max: 500_000_000
        }
    );
    assert!(d.capabilities.triggers.contains(&TriggerKind::RisingEdge));
    assert!(d.capabilities.triggers.contains(&TriggerKind::Level(true)));
    assert_eq!(d.capabilities.buffer_depth, None);
    assert_eq!(*d, virtual_descriptor());
    // Still exactly one after several scans.
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(ids(&registry), vec!["virtual:0"]);
}

#[test]
fn hardware_devices_are_listed_in_the_same_shape() {
    let registry = registry_fast();
    let provider = TestProvider::new("usb");
    let a = descriptor("usb", "A", 8);
    let b = descriptor("usb", "B", 16);
    provider.plug(a.clone());
    provider.plug(b.clone());
    registry.add_provider(Arc::new(provider.clone()));
    assert!(wait_until(WAIT, || registry.list().len() == 3));
    let list = registry.list();
    for expected in [&a, &b] {
        let found = list.iter().find(|d| d.id == expected.id).unwrap();
        assert_eq!(found, expected);
        assert!(!found.simulated);
        assert!(!found.name.is_empty());
        assert!(found.capabilities.channels >= 1);
    }
}

#[test]
fn identities_are_stable_across_listings_and_replug() {
    let registry = registry_fast();
    let provider = TestProvider::new("usb");
    let d = descriptor("usb", "SN-42", 8);
    provider.plug(d.clone());
    registry.add_provider(Arc::new(provider.clone()));
    assert!(wait_until(WAIT, || registry.list().len() == 2));
    let first = registry.list();
    let second = registry.list();
    assert_eq!(first, second);

    provider.unplug(&d.id);
    assert!(wait_until(WAIT, || registry.list().len() == 1));
    provider.plug(d.clone());
    assert!(wait_until(WAIT, || registry.list().len() == 2));
    let after = registry.list();
    let again = after.iter().find(|x| x.id == d.id).unwrap();
    assert_eq!(
        again.id, d.id,
        "the same physical device keeps its identity"
    );
    assert_eq!(DeviceId::new("usb", "SN-42"), d.id);
}

#[test]
fn describe_returns_the_descriptor_or_none() {
    let registry = registry_fast();
    let virtual_id = DeviceId::new("virtual", "0");
    assert_eq!(registry.describe(&virtual_id), Some(virtual_descriptor()));
    assert_eq!(registry.describe(&DeviceId::new("usb", "nope")), None);
}

#[test]
fn an_invalid_descriptor_is_omitted_while_healthy_devices_are_listed() {
    let registry = registry_fast();
    let provider = TestProvider::new("usb");
    let good = descriptor("usb", "good", 8);
    let mut zero_channels = descriptor("usb", "zero", 8);
    zero_channels.capabilities.channels = 0;
    let mut no_rates = descriptor("usb", "norates", 8);
    no_rates.capabilities.sample_rates = SampleRates::List(Vec::new());
    provider.plug(zero_channels);
    provider.plug(no_rates);
    provider.plug(good.clone());
    registry.add_provider(Arc::new(provider.clone()));
    assert!(wait_until(WAIT, || ids(&registry).contains(&good.id.to_string())));
    let listed = ids(&registry);
    assert_eq!(listed, vec!["virtual:0".to_owned(), good.id.to_string()]);
}

#[test]
fn hanging_and_failing_providers_are_omitted_and_list_stays_instant() {
    let registry = registry_fast();
    let healthy = TestProvider::new("healthy");
    healthy.plug(descriptor("healthy", "1", 8));
    let hanging = TestProvider::new("hanging");
    hanging.plug(descriptor("hanging", "1", 8));
    hanging.set_scan_hang(true);
    let failing = TestProvider::new("failing");
    failing.plug(descriptor("failing", "1", 8));
    failing.set_scan_error(true);
    registry.add_provider(Arc::new(healthy.clone()));
    registry.add_provider(Arc::new(hanging.clone()));
    registry.add_provider(Arc::new(failing.clone()));

    // The healthy device appears even though two providers misbehave.
    assert!(wait_until(WAIT, || ids(&registry).contains(&"healthy:1".to_owned())));
    assert!(wait_until(WAIT, || hanging.scan_calls() > 0
        && failing.scan_calls() > 0));

    // `list` never waits for a device, however often it is called while a scan hangs.
    for _ in 0..200 {
        let started = Instant::now();
        let list = registry.list();
        assert!(
            started.elapsed() < Duration::from_millis(10),
            "list took {:?}",
            started.elapsed()
        );
        assert!(list.iter().all(|d| d.id.provider() != "hanging"));
        assert!(list.iter().all(|d| d.id.provider() != "failing"));
        assert!(list.iter().any(|d| d.id.as_str() == "virtual:0"));
    }

    // A hanging provider does not pile up scan threads: at most one scan is in flight.
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        hanging.scan_calls(),
        1,
        "no new scan while the old one hangs"
    );

    // The failing provider recovers once it works again.
    failing.set_scan_error(false);
    assert!(wait_until(WAIT, || ids(&registry).contains(&"failing:1".to_owned())));
    hanging.release();
}

#[test]
fn the_list_is_sorted_virtual_first_then_by_id() {
    let registry = registry_fast();
    let provider = TestProvider::new("usb");
    for unique in ["c", "a", "b"] {
        provider.plug(descriptor("usb", unique, 4));
    }
    let other = TestProvider::new("aaa");
    other.plug(descriptor("aaa", "z", 4));
    registry.add_provider(Arc::new(provider.clone()));
    registry.add_provider(Arc::new(other.clone()));
    assert!(wait_until(WAIT, || registry.list().len() == 5));
    assert_eq!(
        ids(&registry),
        vec!["virtual:0", "aaa:z", "usb:a", "usb:b", "usb:c"]
    );
}
