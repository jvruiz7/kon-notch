//! Sistema: CPU, memória, disco C: e os processos mais pesados (lidos a cada 2 s numa thread).

use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub struct Proc { pub name: String, pub mem_mb: u64, pub cpu: f32 }

#[derive(Clone, Debug, Default)]
pub struct SysInfo {
    pub cpu: f32, pub cores: usize,
    pub mem_used: u64, pub mem_total: u64,
    pub disk_free: u64, pub disk_total: u64,
    pub top: Vec<Proc>,
    pub uptime_h: u64,
}

pub fn start(active: Arc<std::sync::atomic::AtomicBool>, repaint: impl Fn() + Send + 'static) -> Arc<Mutex<SysInfo>> {
    let st = Arc::new(Mutex::new(SysInfo::default()));
    let s2 = st.clone();
    std::thread::spawn(move || {
        use sysinfo::{Disks, ProcessesToUpdate, System};
        let mut sys = System::new();
        let mut disks = Disks::new_with_refreshed_list();
        loop {
            // só mede quando a página Sistema está aberta (medir custa CPU)
            if active.load(std::sync::atomic::Ordering::Relaxed) {
                sys.refresh_cpu_usage();
                sys.refresh_memory();
                sys.refresh_processes(ProcessesToUpdate::All, true);
                disks.refresh(true);
                let mut agg: std::collections::HashMap<String, Proc> = Default::default();
                for p in sys.processes().values() {
                    let name = p.name().to_string_lossy().trim_end_matches(".exe").to_string();
                    let e = agg.entry(name.clone()).or_insert(Proc { name, ..Default::default() });
                    e.mem_mb += p.memory() / 1_048_576;
                    e.cpu += p.cpu_usage();
                }
                let cores = sys.cpus().len().max(1);
                let mut top: Vec<Proc> = agg.into_values().map(|mut p| { p.cpu /= cores as f32; p }).collect();
                // empate em 0% de CPU (1ª leitura): os que mais usam memória
                top.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal).then(b.mem_mb.cmp(&a.mem_mb)));
                top.truncate(4);
                let c = disks.iter().find(|d| d.mount_point().to_string_lossy().to_uppercase().starts_with("C:")).or(disks.iter().next());
                *s2.lock().unwrap() = SysInfo {
                    cpu: sys.global_cpu_usage(), cores,
                    mem_used: sys.used_memory(), mem_total: sys.total_memory(),
                    disk_free: c.map_or(0, |d| d.available_space()), disk_total: c.map_or(0, |d| d.total_space()),
                    top, uptime_h: System::uptime() / 3600,
                };
                repaint();
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    });
    st
}
