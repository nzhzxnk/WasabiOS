extern crate alloc;

use crate::result::Result;
use crate::slice::Sliceable;
use crate::xhci::CommandRing;
use crate::xhci::Controller;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomPinned;
use core::mem::size_of;

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
#[non_exhaustive]
#[allow(unused)]
#[derive(PartialEq, Eq)]
pub enum UsbDescriptorType {
    Device = 1,
    Config = 2,
    String = 3,
    Interface = 4,
    Endpoint = 5,
    Hid = 0x21,
    Report = 0x22,
}

#[derive(Debug, Copy, Clone)]
pub enum UsbDescriptor {
    Config(ConfigDescriptor),
    Endpoint(EndpointDescriptor),
    Interface(InterfaceDescriptor),
    Hid(HidDescriptor),
    Unknown { desc_len: u8, desc_type: u8 },
}

#[derive(Debug, Copy, Clone, Default)]
#[allow(unused)]
#[repr(packed)]
pub struct UsbDeviceDescriptor {
    pub desc_length: u8,
    pub desc_type: u8,
    pub version: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
    pub max_packet_size: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_version: u16,
    pub manufacturer_idx: u8,
    pub product_idx: u8,
    pub serial_idx: u8,
    pub num_of_config: u8,
}
const _: () = assert!(size_of::<UsbDeviceDescriptor>() == 18);
unsafe impl Sliceable for UsbDeviceDescriptor {}

#[derive(Debug, Copy, Clone, Default)]
#[allow(unused)]
#[repr(packed)]
pub struct ConfigDescriptor {
    desc_length: u8,
    desc_type: u8,
    total_length: u16,
    num_of_interfaces: u8,
    config_value: u8,
    config_string_index: u8,
    attribute: u8,
    max_power: u8,
    //
    _pinned: PhantomPinned,
}
const _: () = assert!(size_of::<ConfigDescriptor>() == 9);
impl ConfigDescriptor {
    pub fn total_length(&self) -> usize {
        self.total_length as usize
    }
    pub fn config_value(&self) -> u8 {
        self.config_value
    }
}
unsafe impl Sliceable for ConfigDescriptor {}

pub struct DescriptorIterator<'a> {
    buf: &'a [u8],
    index: usize,
}
impl<'a> DescriptorIterator<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, index: 0 }
    }
}
impl<'a> Iterator for DescriptorIterator<'a> {
    type Item = UsbDescriptor;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.buf.len() {
            None
        } else {
            let buf = &self.buf[self.index..];
            let desc_len = buf[0];
            let desc_type = buf[1];
            let desc = match desc_type {
                e if e == UsbDescriptorType::Config as u8 => {
                    UsbDescriptor::Config(
                        ConfigDescriptor::copy_from_slice(buf).ok()?,
                    )
                }
                e if e == UsbDescriptorType::Interface as u8 => {
                    UsbDescriptor::Interface(
                        InterfaceDescriptor::copy_from_slice(buf).ok()?,
                    )
                }
                e if e == UsbDescriptorType::Endpoint as u8 => {
                    UsbDescriptor::Endpoint(
                        EndpointDescriptor::copy_from_slice(buf).ok()?,
                    )
                }
                e if e == UsbDescriptorType::Hid as u8 => UsbDescriptor::Hid(
                    HidDescriptor::copy_from_slice(buf).ok()?,
                ),
                _ => UsbDescriptor::Unknown {
                    desc_len,
                    desc_type,
                },
            };
            self.index += desc_len as usize;
            Some(desc)
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
#[allow(unused)]
#[repr(packed)]
pub struct InterfaceDescriptor {
    desc_length: u8,
    desc_type: u8,
    pub interface_number: u8,
    pub alt_setting: u8,
    num_of_endpoints: u8,
    interface_class: u8,
    interface_subclass: u8,
    interface_protocol: u8,
    interface_index: u8,
}
const _: () = assert!(size_of::<InterfaceDescriptor>() == 9);
unsafe impl Sliceable for InterfaceDescriptor {}
impl InterfaceDescriptor {
    pub fn triple(&self) -> (u8, u8, u8) {
        (
            self.interface_class,
            self.interface_subclass,
            self.interface_protocol,
        )
    }
}

#[derive(Debug, Copy, Clone, Default)]
#[allow(unused)]
#[repr(packed)]
pub struct EndpointDescriptor {
    pub desc_length: u8,
    pub desc_type: u8,

    // endpoint_address:
    //   - bit[0..=3]: endpoint number
    //   - bit[7]: direction(0: out, 1: in)
    pub endpoint_address: u8,

    // attributes:
    //   - bit[0..=1]: transfer type(0: Control, 1: Isochronous, 2: Bulk, 3:
    //     Interrupt)
    pub attributes: u8,
    pub max_packet_size: u16,
    // interval:
    // [xhci] Table 6-12
    // interval_ms = interval (For FS/LS Interrupt)
    // interval_ms = 2^(interval-1) (For FS Isoch)
    // interval_ms = 2^(interval-1) (For SSP/SS/HS)
    pub interval: u8,
}
const _: () = assert!(size_of::<EndpointDescriptor>() == 7);
unsafe impl Sliceable for EndpointDescriptor {}

// [hid_1_11]:
// 7.2.5 Get_Protocol Request
// 7.2.6 Set_Protocol Request
#[repr(u8)]
pub enum UsbHidProtocol {
    BootProtocol = 0,
}

pub async fn request_device_descriptor(
    xhc: &Rc<Controller>,
    slot: u8,
    ctrl_ep_ring: &mut CommandRing,
) -> Result<UsbDeviceDescriptor> {
    let buf = vec![0; size_of::<UsbDeviceDescriptor>()];
    let mut buf = Box::into_pin(buf.into_boxed_slice());
    xhc.request_descriptor(
        slot,
        ctrl_ep_ring,
        UsbDescriptorType::Device,
        0,
        0,
        &mut buf,
    )
    .await?;
    UsbDeviceDescriptor::copy_from_slice(buf.as_ref().get_ref())
}
pub async fn request_string_descriptor(
    xhc: &Rc<Controller>,
    slot: u8,
    ctrl_ep_ring: &mut CommandRing,
    lang_id: u16,
    index: u8,
) -> Result<String> {
    let buf = vec![0; 128];
    let mut buf = Box::into_pin(buf.into_boxed_slice());
    xhc.request_descriptor(
        slot,
        ctrl_ep_ring,
        UsbDescriptorType::String,
        index,
        lang_id,
        &mut buf,
    )
    .await?;
    Ok(String::from_utf8_lossy(&buf[2..])
        .to_string()
        .replace('\0', ""))
}

pub async fn request_string_descriptor_zero(
    xhc: &Rc<Controller>,
    slot: u8,
    ctrl_ep_ring: &mut CommandRing,
) -> Result<Vec<u8>> {
    let buf = vec![0; 8];
    let mut buf = Box::into_pin(buf.into_boxed_slice());
    xhc.request_descriptor(
        slot,
        ctrl_ep_ring,
        UsbDescriptorType::String,
        0,
        0,
        &mut buf,
    )
    .await?;
    Ok(buf.as_ref().get_ref().to_vec())
}
pub async fn request_config_descriptor_and_rest(
    xhc: &Rc<Controller>,
    slot: u8,
    ctrl_ep_ring: &mut CommandRing,
) -> Result<Vec<UsbDescriptor>> {
    let buf = vec![0u8; size_of::<ConfigDescriptor>()];
    let mut buf = Box::into_pin(buf.into_boxed_slice());
    xhc.request_descriptor(
        slot,
        ctrl_ep_ring,
        UsbDescriptorType::Config,
        0,
        0,
        &mut buf,
    )
    .await?;
    let config_descriptor =
        ConfigDescriptor::copy_from_slice(buf.as_ref().get_ref())?;
    let buf = vec![0; config_descriptor.total_length()];
    let mut buf = Box::into_pin(buf.into_boxed_slice());
    xhc.request_descriptor(
        slot,
        ctrl_ep_ring,
        UsbDescriptorType::Config,
        0,
        0,
        &mut buf,
    )
    .await?;
    let iter = DescriptorIterator::new(&buf);
    let descriptors: Vec<UsbDescriptor> = iter.collect();
    Ok(descriptors)
}
pub async fn request_hid_report(
    xhc: &Rc<Controller>,
    slot: u8,
    ctrl_ep_ring: &mut CommandRing,
) -> Result<Vec<u8>> {
    let buf = vec![0u8; 8];
    let mut buf = Box::into_pin(buf.into_boxed_slice());
    xhc.request_report_bytes(slot, ctrl_ep_ring, &mut buf)
        .await?;
    Ok(buf.to_vec())
}

pub fn pick_interface_with_triple(
    descriptors: &[UsbDescriptor],
    triple: (u8, u8, u8),
) -> Option<(ConfigDescriptor, InterfaceDescriptor, Vec<UsbDescriptor>)> {
    let mut config: Option<ConfigDescriptor> = None;
    let mut interface: Option<InterfaceDescriptor> = None;
    let mut desc_list: Vec<UsbDescriptor> = Vec::new();
    for d in descriptors {
        match d {
            UsbDescriptor::Config(e) => {
                if interface.is_some() {
                    break;
                }
                config = Some(*e);
                desc_list.clear();
            }
            UsbDescriptor::Interface(e) => {
                if triple == e.triple() {
                    interface = Some(*e)
                }
            }
            e => {
                if interface.is_some() {
                    desc_list.push(*e)
                }
            }
        }
    }
    if let (Some(config), Some(interface)) = (config, interface) {
        Some((config, interface, desc_list))
    } else {
        None
    }
}
pub async fn request_hid_report_descriptor(
    xhc: &Rc<Controller>,
    slot: u8,
    ctrl_ep_ring: &mut CommandRing,
    interface_number: u8,
    desc_size: usize,
) -> Result<Vec<u8>> {
    // 7.1.1 Get_Descriptor Request
    let buf = vec![0u8; desc_size];
    let mut buf = Box::into_pin(buf.into_boxed_slice());
    xhc.request_descriptor_for_interface(
        slot,
        ctrl_ep_ring,
        UsbDescriptorType::Report,
        0,
        interface_number.into(),
        &mut buf,
    )
    .await?;
    Ok((*buf).to_vec())
}
#[derive(Debug, Copy, Clone, Default)]
#[allow(unused)]
#[repr(packed)]
pub struct HidDescriptor {
    desc_length: u8,
    desc_type: u8,
    hid_release: u16,
    country_code: u8,
    num_descriptors: u8,
    descriptor_type: u8,
    pub report_descriptor_length: u16,
}
const _: () = assert!(size_of::<HidDescriptor>() == 9);
unsafe impl Sliceable for HidDescriptor {}

pub trait UsbDeviceDriver {
    fn is_compatible(
        descriptors: &[UsbDescriptor],
        device_descriptor: &UsbDeviceDescriptor,
    ) -> bool;
    fn start(
        xhc: Rc<Controller>,
        slot: u8,
        ctrl_ep_ring: CommandRing,
        descriptors: Vec<UsbDescriptor>,
    );
}
