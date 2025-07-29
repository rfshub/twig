// src/modules/ip/lookup.rs

use axum::response::Response;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use futures::future;
use ip_lookup::{get_public_ip_addr, lookup, LookupProvider, LookupResult};
use once_cell::sync::Lazy;
use rand::seq::SliceRandom;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::time::timeout;
use crate::core::response;

struct CachedIp {
    ip: String,
    updated_at: DateTime<Utc>,
}

struct CachedGeoIp {
    data: Value,
    updated_at: DateTime<Utc>,
}

static LAST_IP: Lazy<Mutex<Option<CachedIp>>> = Lazy::new(|| Mutex::new(None));
static LAST_GEOIP: Lazy<Mutex<Option<CachedGeoIp>>> = Lazy::new(|| Mutex::new(None));

pub async fn get_ip_handler() -> Response {
    let now = Utc::now();
    let use_cached = {
        let guard = LAST_IP.lock().unwrap();
        guard.as_ref().map_or(false, |cached| now - cached.updated_at < Duration::minutes(15))
    };

    if use_cached {
        let guard = LAST_IP.lock().unwrap();
        if let Some(cached) = &*guard {
            return response::success(Some(json!({
                "ip": cached.ip,
                "update": cached.updated_at.to_rfc3339_opts(SecondsFormat::Secs, true)
            })));
        }
    }

    let result = tokio::task::spawn_blocking(move || get_public_ip_addr())
        .await
        .unwrap_or(None);

    match result {
        Some(ip) => {
            let updated_at = Utc::now();
            let mut guard = LAST_IP.lock().unwrap();
            *guard = Some(CachedIp {
                ip: ip.clone(),
                updated_at,
            });

            response::success(Some(json!({
                "ip": ip,
                "update": updated_at.to_rfc3339_opts(SecondsFormat::Secs, true)
            })))
        }
        None => response::service_unavailable(),
    }
}

pub async fn get_geoip_handler() -> Response {
    let now = Utc::now();
    {
        let guard = LAST_GEOIP.lock().unwrap();
        if let Some(cached) = &*guard {
            if now - cached.updated_at < Duration::minutes(15) {
                return response::success(Some(cached.data.clone()));
            }
        }
    }

    let providers = vec![
        LookupProvider::IpApi,
        LookupProvider::IpInfo,
        LookupProvider::IpSb,
        LookupProvider::IpApiIo,
        LookupProvider::ApipCc,
        LookupProvider::IpapiIs,
        LookupProvider::Geolocated,
        LookupProvider::IpLocationApi,
    ];

    let lookups_as_futures = providers.into_iter().map(|p| {
        Box::pin(async move {
            match tokio::task::spawn_blocking(move || lookup(p)).await {
                Ok(Some(data)) => Ok(data),
                _ => Err(()),
            }
        })
    });

    let results = match timeout(std::time::Duration::from_secs(10), future::join_all(lookups_as_futures)).await {
        Ok(res) => res,
        Err(_) => vec![], // On timeout, we might have no results.
    };

    let successful_lookups: Vec<LookupResult> = results.into_iter().filter_map(Result::ok).collect();
    if successful_lookups.is_empty() {
        return response::service_unavailable();
    }

    // Build the raw data object by merging all results
    let mut data_to_process = initial_data_from_response(&successful_lookups[0]);
    for res in successful_lookups.iter().skip(1) {
        merge_data(&mut data_to_process, res);
    }

    // Run the optimization pass on the collected data
    run_optimized_result(&mut data_to_process);

    // Update the cache with the final, optimized data
    {
        let mut guard = LAST_GEOIP.lock().unwrap();
        *guard = Some(CachedGeoIp {
            data: data_to_process.clone(),
            updated_at: Utc::now(),
        });
    }

    response::success(Some(data_to_process))
}

// Main optimization dispatcher. Now operates on a mutable Value.
fn run_optimized_result(data: &mut Value) {
    // Process Country fields
    if let Some(country) = data.get_mut("country") {
        if let Some(map) = country.as_object_mut() {
            for (key, val) in map.iter_mut() {
                match key.as_str() {
                    "city" | "zip" => deduplicate_and_sort_by_frequency(val),
                    _ => consolidate_to_most_frequent(val),
                }
            }
        }
    }

    // Process Location with its special averaging rule
    if let Some(location) = data.get_mut("location") {
        optimize_location(location);
    }

    // Process all other top-level objects with the standard consolidation rule.
    for key in ["network", "connection"] {
        if let Some(obj) = data.get_mut(key) {
            if let Some(obj_map) = obj.as_object_mut() {
                for (_, field_val) in obj_map.iter_mut() {
                    consolidate_to_most_frequent(field_val);
                }
            }
        }
    }
}

// Reduces an array to its most frequent element. On tie, chooses randomly.
fn consolidate_to_most_frequent(field: &mut Value) {
    if let Some(arr) = field.as_array() {
        if arr.is_empty() {
            *field = Value::Null;
            return;
        }
        let mut counts = HashMap::new();
        for val in arr {
            *counts.entry(val).or_insert(0) += 1;
        }
        let max_count = counts.values().max().cloned().unwrap_or(0);
        let winners: Vec<_> = counts.into_iter().filter(|(_, count)| *count == max_count).map(|(val, _)| val.clone()).collect();
        if let Some(winner) = winners.choose(&mut rand::thread_rng()) {
            *field = winner.clone();
        } else {
            *field = Value::Null;
        }
    }
}

fn deduplicate_and_sort_by_frequency(field: &mut Value) {
    if let Some(arr) = field.as_array() {
        if arr.is_empty() {
            return;
        }
        let mut counts = HashMap::new();
        for val in arr {
            if let Some(s) = val.as_str() {
                if s.trim().is_empty() {
                    continue;
                }
            }
            *counts.entry(val.clone()).or_insert(0) += 1;
        }
        if counts.is_empty() {
            *field = Value::Null;
            return;
        }
        let mut sorted_unique_vals: Vec<_> = counts.into_iter().collect();
        sorted_unique_vals.sort_by(|a, b| b.1.cmp(&a.1));
        *field = json!(sorted_unique_vals.into_iter().map(|(val, _)| val).collect::<Vec<_>>());
    }
}

// Averages location coordinates after removing outliers.
fn optimize_location(location_val: &mut Value) {
    if location_val.is_object() {
        if let Some(lat) = location_val.get_mut("latitude") {
            average_coordinate_without_outliers(lat);
        }
        if let Some(lon) = location_val.get_mut("longitude") {
            average_coordinate_without_outliers(lon);
        }
    }
}

fn average_coordinate_without_outliers(field: &mut Value) {
    if let Some(arr) = field.as_array() {
        let mut nums: Vec<f64> = arr.iter().filter_map(|v| v.as_f64()).collect();
        if nums.is_empty() {
            *field = Value::Null;
            return;
        }
        if nums.len() <= 3 {
            let avg = nums.iter().sum::<f64>() / nums.len() as f64;
            *field = json!(avg);
            return;
        }

        nums.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let q1_idx = (nums.len() as f64 * 0.25).floor() as usize;
        let q3_idx = (nums.len() as f64 * 0.75).floor() as usize;
        let q1 = nums[q1_idx];
        let q3 = nums[q3_idx];
        let iqr = q3 - q1;
        let lower_bound = q1 - 1.5 * iqr;
        let upper_bound = q3 + 1.5 * iqr;
        let filtered_nums: Vec<f64> = nums.into_iter().filter(|&x| x >= lower_bound && x <= upper_bound).collect();

        if filtered_nums.is_empty() {
            *field = json!(q1); // Fallback to a central value if all are outliers
            return;
        }

        let avg = filtered_nums.iter().sum::<f64>() / filtered_nums.len() as f64;
        let factor = 10.0_f64.powi(10);
        *field = json!((avg * factor).round() / factor);
    }
}


fn initial_data_from_response(res: &LookupResult) -> Value {
    let single_val = |v: &Option<String>| v.as_ref().map_or(json!([]), |s| json!([s]));
    let single_val_bool = |v: &Option<bool>| v.as_ref().map_or(json!([]), |b| json!([b]));
    let single_val_f64 = |v: &Option<f64>| v.as_ref().map_or(json!([]), |f| json!([f]));

    json!({
        "network": { "ip": single_val(&res.network.ip), "isp": single_val(&res.network.isp), "org": single_val(&res.network.org), "asn": single_val(&res.network.asn), },
        "country": { "city": single_val(&res.country.city), "code": single_val(&res.country.code), "zip": single_val(&res.country.zip), "timezone": single_val(&res.country.timezone), },
        "location": { "latitude": single_val_f64(&res.location.latitude), "longitude": single_val_f64(&res.location.longitude), },
        "connection": { "is_proxy": single_val_bool(&res.connection.is_proxy), "is_tor": single_val_bool(&res.connection.is_tor), "is_crawler": single_val_bool(&res.connection.is_crawler), "is_datacenter": single_val_bool(&res.connection.is_datacenter), "is_vpn": single_val_bool(&res.connection.is_vpn), }
    })
}

fn merge_data(cached_value: &mut Value, new_res: &LookupResult) {
    let merge_field = |arr: &mut Value, val: &Value| {
        if !val.is_null() {
            if let Some(a) = arr.as_array_mut() {
                a.push(val.clone());
            }
        }
    };

    if let Some(ip) = &new_res.network.ip { merge_field(&mut cached_value["network"]["ip"], &json!(ip)); }
    if let Some(isp) = &new_res.network.isp { merge_field(&mut cached_value["network"]["isp"], &json!(isp)); }
    if let Some(org) = &new_res.network.org { merge_field(&mut cached_value["network"]["org"], &json!(org)); }
    if let Some(asn) = &new_res.network.asn { merge_field(&mut cached_value["network"]["asn"], &json!(asn)); }
    if let Some(city) = &new_res.country.city { merge_field(&mut cached_value["country"]["city"], &json!(city)); }
    if let Some(code) = &new_res.country.code { merge_field(&mut cached_value["country"]["code"], &json!(code)); }
    if let Some(zip) = &new_res.country.zip { merge_field(&mut cached_value["country"]["zip"], &json!(zip)); }
    if let Some(timezone) = &new_res.country.timezone { merge_field(&mut cached_value["country"]["timezone"], &json!(timezone)); }
    if let Some(lat) = new_res.location.latitude { merge_field(&mut cached_value["location"]["latitude"], &json!(lat)); }
    if let Some(lon) = new_res.location.longitude { merge_field(&mut cached_value["location"]["longitude"], &json!(lon)); }
    if let Some(v) = new_res.connection.is_proxy { merge_field(&mut cached_value["connection"]["is_proxy"], &json!(v)); }
    if let Some(v) = new_res.connection.is_tor { merge_field(&mut cached_value["connection"]["is_tor"], &json!(v)); }
    if let Some(v) = new_res.connection.is_crawler { merge_field(&mut cached_value["connection"]["is_crawler"], &json!(v)); }
    if let Some(v) = new_res.connection.is_datacenter { merge_field(&mut cached_value["connection"]["is_datacenter"], &json!(v)); }
    if let Some(v) = new_res.connection.is_vpn { merge_field(&mut cached_value["connection"]["is_vpn"], &json!(v)); }
}