//! Wo die KI liest: auf dem eigenen Rechner, im eigenen Netz oder bei einem
//! fremden Dienst. Rein, ohne Netz.
//!
//! Entschieden von Nemesus am 21.11.2026 ("ein klares Go"): die Karte eines
//! Profils sagt, wohin die Sätze gehen. Sein Gedanke dahinter: ein Raum, in
//! dem kein fremder Dienst mitliest, verdient eine Auszeichnung.
//!
//! Der Fisch urteilt allein nach der Adresse, an die er schickt. Mehr weiß er
//! nicht, und mehr behauptet er nicht.
//!
//! Benannte Grenzen:
//! - Was hinter einer Adresse läuft, sieht der Fisch nicht. Auch ein Programm
//!   auf dem eigenen Rechner kann weiterreichen, was es bekommt.
//! - Ob ein Modell quelloffen ist, lässt sich an keiner Adresse ablesen.
//! - Im Zweifel gilt eine Adresse als fremd. Ein eigener Server im Internet
//!   oder eine Adresse aus einem privaten Überlagerungsnetz erscheint deshalb
//!   als fremder Dienst, obwohl er dem Gastgeber gehören kann.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Wo der Dienst steht, an den der Fisch schickt, was er übersetzen lässt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Dieser Rechner: `localhost` oder eine Adresse, die ihn selbst meint.
    ThisMachine,
    /// Das eigene Netz: eine private Adresse oder ein Name, den nur das
    /// eigene Netz kennt.
    OwnNetwork,
    /// Alles andere: ein Dienst draußen.
    Outside,
}

impl Place {
    /// Das Wort, unter dem der Ort in der Liste der Profile steht.
    pub fn id(self) -> &'static str {
        match self {
            Place::ThisMachine => "this_machine",
            Place::OwnNetwork => "own_network",
            Place::Outside => "outside",
        }
    }
}

/// Ordnet die Adresse eines KI-Dienstes ein, etwa `https://api.example/v1`
/// oder `http://localhost:11434/v1`.
pub fn place_of(address: &str) -> Place {
    let host = host_of(address);
    if host.is_empty() {
        return Place::Outside;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(ip) => place_of_v4(ip),
            IpAddr::V6(ip) => place_of_v6(ip),
        };
    }
    if host == "localhost" || host.ends_with(".localhost") {
        return Place::ThisMachine;
    }
    // Namen, die nur ein eigenes Netz auflöst: ohne Punkt ("nas"), oder mit
    // einer Endung, die es im Internet nicht gibt.
    let own = !host.contains('.')
        || [".local", ".lan", ".internal", ".home.arpa"].iter().any(|end| host.ends_with(end));
    if own { Place::OwnNetwork } else { Place::Outside }
}

fn place_of_v4(ip: Ipv4Addr) -> Place {
    if ip.is_loopback() {
        Place::ThisMachine
    } else if ip.is_private() || ip.is_link_local() {
        Place::OwnNetwork
    } else {
        Place::Outside
    }
}

fn place_of_v6(ip: Ipv6Addr) -> Place {
    // Eine IPv4-Adresse in IPv6-Schreibweise gilt als das, was sie ist.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return place_of_v4(v4);
    }
    let first = ip.segments()[0];
    if ip.is_loopback() {
        Place::ThisMachine
    } else if first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80 {
        // fc00::/7 sind private Adressen, fe80::/10 gelten nur am selben Kabel.
        Place::OwnNetwork
    } else {
        Place::Outside
    }
}

/// Der Rechnername aus einer Adresse, klein geschrieben: ohne Protokoll, ohne
/// Anmeldung, ohne Port und ohne Pfad. Eine IPv6-Adresse ohne ihre Klammern.
fn host_of(address: &str) -> String {
    let address = address.trim();
    let rest = address.split_once("://").map_or(address, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    // Was vor einem @ steht, ist eine Anmeldung, kein Rechner.
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let host = if let Some(inner) = host_port.strip_prefix('[') {
        // [::1]:8080
        inner.split(']').next().unwrap_or("")
    } else if host_port.matches(':').count() == 1 {
        // name:port. Mehr als ein Doppelpunkt ohne Klammern ist IPv6 ohne Port.
        host_port.split(':').next().unwrap_or("")
    } else {
        host_port
    };
    host.trim_end_matches('.').to_lowercase()
}
