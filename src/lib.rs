#![deny(missing_docs)]

//! A library for interoperating with the network interfaces of a system.
//!
//! TODO: add more documentation on how to use.

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate lazy_static;
extern crate libc;
extern crate nix;

use std::fmt;
use std::net;
use libc::c_int;
use std::ptr;

extern crate network_interface;

use network_interface::NetworkInterface;
use network_interface::NetworkInterfaceConfig;

use network_interface::V4IfAddr;
use network_interface::V6IfAddr;

use error::InterfacesError;
use flags::InterfaceFlags;

// mod error;

/// Submodule containing various flags.
pub mod flags;

/// `Kind` represents the interface family (equivalent to the `sa_family` field in the `sockaddr`
/// structure).
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Kind {
    /// This interface is IPv4.
    Ipv4,

    /// This interface is IPv6.
    Ipv6,

    /// This interface is a link interface (`AF_LINK`).
    Link,

    /// This interface has an unknown interface type.  The interior `i32` contains the numerical
    /// value that is unknown.
    Unknown(i32),

    /// Linux only: this interface is a packet interface (`AF_PACKET`).
    Packet,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Kind::Ipv4 => write!(f, "IPv4"),
            Kind::Ipv6 => write!(f, "IPv6"),
            Kind::Link => write!(f, "Link"),
            Kind::Unknown(v) => write!(f, "Unknown({})", v),
            Kind::Packet => write!(f, "Packet"),
        }
    }
}

/// The next hop for an interface.  See the individual variants for more information.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum NextHop {
    /// The broadcast address associated with the interface's address.
    Broadcast(net::SocketAddr),

    /// The destination address of a point-to-point interface.
    Destination(net::SocketAddr),
}

impl fmt::Display for NextHop {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            NextHop::Broadcast(ref addr) => write!(f, "Broadcast({})", addr),
            NextHop::Destination(ref addr) => write!(f, "Destination({})", addr),
        }
    }
}

/// This structure represents a single address for a given interface.
#[derive(Debug, Clone, Copy)]
pub struct Address {
    /// The kind of address this is (e.g. IPv4).
    pub kind: Kind,

    /// The underlying socket address, if it applies.
    pub addr: Option<net::SocketAddr>,

    /// The netmask of this interface address, if it applies.
    pub mask: Option<net::SocketAddr>,

    /// The broadcast address or destination address, if it applies.
    pub hop: Option<NextHop>,
}

fn to_address(addr: &network_interface::Addr) -> Address {
    Address { 
        kind: {
            match addr {
                network_interface::Addr::V4(V4IfAddr) => Kind::Ipv4,
                network_interface::Addr::V6(V6IfAddr) => Kind::Ipv6
            }
        }, 
        addr: Some(net::SocketAddr::new(addr.ip(), 0)), 
        mask: {
            match addr.netmask() {
                Some(ip) => Some(net::SocketAddr::new(ip, 0)),
                None => None
            }
        }, 
        hop: {
            match addr.broadcast() {
                Some(broadcast) => Some(NextHop::Broadcast(net::SocketAddr::new(broadcast, 0))),
                None => None
            }
        }
    }
}

/// HardwareAddr represents a hardware address (commonly known as a MAC address) of a given
/// interface.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct HardwareAddr([u8; 6]);

impl HardwareAddr {
    /// Returns a new, empty `HardwareAddr` structure.  This is equivalent to the MAC address
    /// `00:00:00:00:00:00`.
    pub fn zero() -> HardwareAddr {
        HardwareAddr([0; 6])
    }

    /// Formats this hardware address in the standard MAC address format - 6 octets in hexadecimal
    /// format, each seperated by a colon.
    ///
    /// ```
    /// # use interfaces::HardwareAddr;
    /// let s = HardwareAddr::zero().as_string();
    /// assert_eq!(s, "00:00:00:00:00:00");
    /// ```
    pub fn as_string(&self) -> String {
        let &HardwareAddr(ref arr) = self;

        format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            arr[0], arr[1], arr[2], arr[3], arr[4], arr[5],
        )
    }

    /// Formats this hardware address as a sequence of hexadecimal numbers without the seperating
    /// colons.
    ///
    /// ```
    /// # use interfaces::HardwareAddr;
    /// let s = HardwareAddr::zero().as_bare_string();
    /// assert_eq!(s, "000000000000");
    /// ```
    pub fn as_bare_string(&self) -> String {
        let &HardwareAddr(ref arr) = self;

        format!(
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            arr[0], arr[1], arr[2], arr[3], arr[4], arr[5],
        )
    }

    /// Returns the raw bytes representing this hardware address.
    ///
    /// ```
    /// # use interfaces::HardwareAddr;
    /// let s = HardwareAddr::zero();
    /// assert_eq!(s.as_bytes(), &[0, 0, 0, 0, 0, 0]);
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        let &HardwareAddr(ref arr) = self;
        arr
    }
}

impl fmt::Display for HardwareAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

/// The `Interface` structure represents a single interface on the system.  It also contains
/// methods to control the interface.
#[derive(Debug)]
pub struct Interface {
    /// The name of this interface.
    pub name: String,

    /// All addresses for this interface.
    pub addresses: Vec<Address>,

    /// Interface flags.
    ///
    /// NOTE: The underlying API returns this value for each address of an interface, not each
    /// interface itself.  We assume that they are all equal and take the first set of flags (from
    /// the first address).
    pub flags: InterfaceFlags,

    // Information socket
    sock: c_int,
}

impl Interface {
    /// Retrieve a list of all interfaces on this system.
    pub fn get_all() -> Result<Vec<Interface>> {

        let network_interfaces: Vec<NetworkInterface> = NetworkInterface::show().unwrap();
        let mut res: Vec<Interface> = Vec::new();

        for netif in network_interfaces.iter() {
            let mut addrs: Vec<Address> = Vec::new();
            
            for addr in &netif.addr {
                addrs.push(to_address(&addr));
            }
            let intf = Interface {
                name: netif.name.clone(),
                addresses: addrs,
                flags: InterfaceFlags::IFF_UP,
                sock: 1,
            };
            res.push(intf);
        }

        Ok(res)
    }

}

impl PartialEq for Interface {
    fn eq(&self, other: &Interface) -> bool {
        self.name == other.name
    }
}

impl fmt::Display for Interface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Name: {} \nAddresses {:?}\n", self.name, self.addresses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hash;

    #[test]
    fn test_interface_is_comparable() {
        let ifs = Interface::get_all().unwrap();
        for intf in &ifs{
            println!("{}", intf);
        }
        assert!(ifs[0] == ifs[0]);
    }

    #[test]
    fn test_hardwareaddr_deriving() {
        let one = HardwareAddr::zero();
        let two = HardwareAddr::zero();

        assert!(one == two);
        assert_is_clone(&one);
        assert_is_copy(&one);
        assert_is_hash(&one);
    }

    #[test]
    fn test_hardwareaddr_format() {
        let h = HardwareAddr::zero();

        assert_eq!(h.as_string(), "00:00:00:00:00:00");
        assert_eq!(h.as_bare_string(), "000000000000");
    }

    fn assert_is_clone<T: Clone>(_: &T) {}
    fn assert_is_copy<T: Copy>(_: &T) {}
    fn assert_is_hash<T: Hash>(_: &T) {}
}
