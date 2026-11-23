use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let args: Vec<String> = env::args().collect();
    let bin_name = Path::new(args.first().map(String::as_str).unwrap_or("hy2ctl"))
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("hy2ctl")
        .to_string();

    // Симлинк hy2auth вызывает Hysteria как command-auth: <addr> <auth> <tx>
    if bin_name == "hy2auth" {
        process::exit(cmd_auth(&args[1..]));
    }

    let result = match args.get(1).map(String::as_str) {
        None | Some("start") => cmd_start(),
        Some("auth") => process::exit(cmd_auth(&args[2..])),
        Some("keygen") => cmd_keygen(&args[2..]),
        Some("geo-update") | Some("geo_update") => cmd_geo_update(&args[2..]),
        Some("users") => cmd_users(&args[2..]),
        Some("help") | Some("-h") | Some("--help") => {
            print_help();
            Ok(())
        }
        Some(other) => Err(format!("неизвестная команда: {}", other)),
    };

    if let Err(e) = result {
        eprintln!("Ошибка: {}", e);
        process::exit(1);
    }
}

fn print_help() {
    println!(
        "hy2ctl — Rust-обвязка для Hysteria 2\n\n\
         Команды:\n\
           start                  подготовить базы/сертификаты и запустить hysteria\n\
           auth <addr> <auth> <tx> command-auth (обычно вызывается через /usr/local/bin/hy2auth)\n\
           keygen <username>      напечатать клиентский URI hysteria2://...\n\
           geo-update [geoip|geosite]  обновить базы GeoIP/GeoSite\n\
           users add|pass|del|list    управление пользователями\n\
           help                   эта справка"
    );
}

// ---------------------------------------------------------------- helpers

fn clean(s: &str) -> String {
    s.trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim()
        .to_string()
}

fn script_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Переменная окружения имеет приоритет, потом server.env рядом с бинарником и /etc/hysteria.
fn env_or_file(key: &str, dir: &Path) -> Option<String> {
    if let Ok(v) = env::var(key) {
        let v = clean(&v);
        if !v.is_empty() {
            return Some(v);
        }
    }
    for path in [dir.join("server.env"), PathBuf::from("/etc/hysteria/server.env")] {
        if let Ok(text) = fs::read_to_string(&path) {
            for line in text.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    if clean(k) == key {
                        let v = clean(v);
                        if !v.is_empty() {
                            return Some(v);
                        }
                    }
                }
            }
        }
    }
    None
}

fn users_path(dir: &Path) -> PathBuf {
    if let Ok(p) = env::var("USERS_FILE") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    // В контейнере конфиг и пользователи всегда живут в /etc/hysteria —
    // даже если users.txt ещё не создан (создаст start).
    if Path::new("/etc/hysteria/config.yaml").exists() || Path::new("/etc/hysteria").is_dir() {
        return PathBuf::from("/etc/hysteria/users.txt");
    }
    dir.join("users.txt")
}

fn config_path(dir: &Path) -> PathBuf {
    if let Ok(p) = env::var("CONFIG_FILE") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    let system = PathBuf::from("/etc/hysteria/config.yaml");
    if system.exists() {
        return system;
    }
    dir.join("config.yaml")
}

fn geo_dir(dir: &Path) -> PathBuf {
    if let Ok(p) = env::var("GEO_DIR") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    if Path::new("/etc/hysteria/config.yaml").exists() {
        return PathBuf::from("/etc/hysteria");
    }
    dir.to_path_buf()
}

/// RFC 3986 percent-encoding: оставляем только unreserved-символы.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn find_user(path: &Path, name: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some((user, pass)) = t.split_once(':') {
            if user.trim().eq_ignore_ascii_case(name) {
                return Some(pass.to_string());
            }
        }
    }
    None
}

fn valid_username(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn random_password() -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
        ^ (process::id() as u64).wrapping_mul(0x9E3779B97F4A7C15);
    let mut x = seed;
    let mut out = String::with_capacity(28);
    for _ in 0..28 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.push(CHARS[(x as usize) % CHARS.len()] as char);
    }
    out
}

// ---------------------------------------------------------------- geo update

fn urls_for(kind: &str, base: &str, cdn: &str, explicit: Option<String>) -> Vec<String> {
    if let Some(e) = explicit {
        let list: Vec<String> = e
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !list.is_empty() {
            return list;
        }
    }
    vec![format!("{}/{}", base, kind), format!("{}/{}", cdn, kind)]
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| format!("{}: {}", url, e))?;
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| format!("{}: {}", url, e))?;
    Ok(buf)
}

fn validate_dat(buf: &[u8]) -> bool {
    buf.len() > 100_000 && buf.first() == Some(&0x0A)
}

fn update_geo(kind: &str, dir: &Path, urls: &[String]) -> Result<PathBuf, String> {
    let target = dir.join(format!("{}.dat", kind));
    let mut last_err = String::new();

    for url in urls {
        println!("Скачиваем {}: {}", kind, url);
        match download(url) {
            Ok(buf) if validate_dat(&buf) => {
                if target.exists() {
                    let _ = fs::copy(&target, target.with_extension("dat.bak"));
                }
                let tmp = target.with_extension("dat.tmp");
                fs::write(&tmp, &buf).map_err(|e| e.to_string())?;
                if target.exists() {
                    let _ = fs::remove_file(&target);
                }
                fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
                println!(
                    "Обновлён: {} ({} байт)",
                    target.display(),
                    buf.len()
                );
                return Ok(target);
            }
            Ok(_) => last_err = format!("{}: файл не прошёл проверку", url),
            Err(e) => last_err = e,
        }
        eprintln!("Источник не сработал, пробуем следующий...");
    }
    Err(format!(
        "ни один источник не отдал корректный {}.dat: {}",
        kind, last_err
    ))
}

fn cmd_geo_update(args: &[String]) -> Result<(), String> {
    let dir = geo_dir(&script_dir());
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let kinds: Vec<&str> = match args.first().map(String::as_str) {
        None => vec!["geoip", "geosite"],
        Some("geoip") => vec!["geoip"],
        Some("geosite") => vec!["geosite"],
        Some(other) => return Err(format!("неизвестная база: {}", other)),
    };

    let base = env::var("GEO_BASE_URL")
        .unwrap_or_else(|_| "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download".into());
    let cdn = env::var("GEO_CDN_BASE")
        .unwrap_or_else(|_| "https://cdn.jsdelivr.net/gh/Loyalsoldier/v2ray-rules-dat@release".into());

    for kind in kinds {
        let explicit = env::var(if kind == "geoip" { "GEOIP_URL" } else { "GEOSITE_URL" }).ok();
        let urls = urls_for(
            &format!("{}.dat", kind),
            &base,
            &cdn,
            explicit,
        );
        update_geo(kind, &dir, &urls)?;
    }
    println!("Готово. Перезапустите hysteria/контейнер, чтобы применить базы.");
    Ok(())
}

// ---------------------------------------------------------------- auth

fn cmd_auth(args: &[String]) -> i32 {
    if args.len() < 2 {
        return 1;
    }
    let auth = &args[1];
    let (name, pass) = match auth.split_once(':') {
        Some(v) if !v.0.is_empty() && !v.1.is_empty() => v,
        _ => return 1,
    };

    let path = users_path(&script_dir());
    match find_user(&path, name) {
        Some(stored) if stored == pass => {
            println!("{}", stored_user_name(&path, name).unwrap_or_else(|| name.to_string()));
            0
        }
        _ => 1,
    }
}

fn stored_user_name(path: &Path, name: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some((user, _)) = t.split_once(':') {
            if user.trim().eq_ignore_ascii_case(name) {
                return Some(user.trim().to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------- keygen

fn masq_host(config: &Path) -> Option<String> {
    let text = fs::read_to_string(config).ok()?;
    let mut in_masq = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "masquerade:" {
            in_masq = true;
            continue;
        }
        let indented = line.chars().next().map_or(false, |c| c.is_whitespace());
        if in_masq && !trimmed.is_empty() && !trimmed.starts_with('#') && !indented {
            in_masq = false;
        }
        if !in_masq {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("url:") {
            let mut s = clean(rest);
            if let Some(pos) = s.find("://") {
                s = s[pos + 3..].to_string();
            }
            s = s
                .split(|c| c == '/' || c == '?' || c == '#')
                .next()
                .unwrap_or("")
                .to_string();
            if let Some(pos) = s.rfind('@') {
                s = s[pos + 1..].to_string();
            }
            if let Some(pos) = s.rfind(':') {
                if s[pos + 1..].chars().all(|c| c.is_ascii_digit()) {
                    s = s[..pos].to_string();
                }
            }
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

fn cmd_keygen(args: &[String]) -> Result<(), String> {
    let username = args
        .first()
        .ok_or_else(|| "использование: hy2ctl keygen <username>".to_string())?;
    let dir = script_dir();
    let users = users_path(&dir);
    let password = find_user(&users, username)
        .ok_or_else(|| format!("пользователь '{}' не найден в {}", username, users.display()))?;
    if password.is_empty() {
        return Err(format!("у пользователя '{}' пустой пароль", username));
    }

    let host = env_or_file("SERVER_IP", &dir)
        .ok_or_else(|| "SERVER_IP не задан в env/server.env".to_string())?;
    let port = env_or_file("SERVER_PORT", &dir).unwrap_or_else(|| "443".to_string());
    if port.parse::<u16>().map_or(true, |p| p == 0) {
        return Err(format!("некорректный SERVER_PORT: {}", port));
    }

    let sni = env::var("SNI")
        .ok()
        .map(|s| clean(&s))
        .filter(|s| !s.is_empty())
        .or_else(|| masq_host(&config_path(&dir)))
        .unwrap_or_else(|| host.clone());

    let authority = if host.contains(':') && !host.starts_with('[') && !host.ends_with(']') {
        format!("[{}]", host)
    } else {
        host.clone()
    };

    println!(
        "hysteria2://{}:{}@{}:{}/?insecure=1&sni={}#Hysteria2",
        url_encode(username),
        url_encode(&password),
        url_encode(&authority).replace("%5B", "[").replace("%5D", "]"),
        port,
        url_encode(&sni)
    );
    Ok(())
}

// ---------------------------------------------------------------- users

fn read_users(path: &Path) -> Result<Vec<String>, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text.lines().map(|l| l.trim_end_matches('\r').to_string()).collect()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.to_string()),
    }
}

fn save_users(path: &Path, lines: &[String]) -> Result<(), String> {
    let tmp = path.with_extension("txt.tmp");
    let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
    for line in lines {
        writeln!(f, "{}", line).map_err(|e| e.to_string())?;
    }
    drop(f);
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn cmd_users(args: &[String]) -> Result<(), String> {
    let path = users_path(&script_dir());
    match args.first().map(String::as_str) {
        Some("list") => {
            for line in read_users(&path)? {
                if !line.trim().is_empty() {
                    println!("{}", line);
                }
            }
            Ok(())
        }
        Some("add") => {
            let name = args
                .get(1)
                .ok_or_else(|| "использование: users add <name> [password]".to_string())?;
            if !valid_username(name) {
                return Err("имя пользователя: только буквы, цифры, _ и -".into());
            }
            let mut lines = read_users(&path)?;
            if lines.iter().any(|l| {
                l.split_once(':')
                    .map_or(false, |(u, _)| u.trim().eq_ignore_ascii_case(name))
            }) {
                return Err(format!("пользователь '{}' уже существует", name));
            }
            let pass = match args.get(2) {
                Some(p) if !p.is_empty() => p.clone(),
                _ => random_password(),
            };
            if pass.contains(':') || pass.chars().any(char::is_whitespace) {
                return Err("пароль не должен содержать ':' или пробелы".into());
            }
            println!("Пароль: {}", pass);
            lines.push(format!("{}:{}", name, pass));
            save_users(&path, &lines)
        }
        Some("pass") => {
            let name = args
                .get(1)
                .ok_or_else(|| "использование: users pass <name> [password]".to_string())?;
            let mut lines = read_users(&path)?;
            let mut found = false;
            let pass = match args.get(2) {
                Some(p) if !p.is_empty() => p.clone(),
                _ => random_password(),
            };
            for line in lines.iter_mut() {
                if let Some((u, _)) = line.split_once(':') {
                    if u.trim().eq_ignore_ascii_case(name) {
                        *line = format!("{}:{}", u.trim(), pass);
                        found = true;
                    }
                }
            }
            if !found {
                return Err(format!("пользователь '{}' не найден", name));
            }
            println!("Новый пароль: {}", pass);
            save_users(&path, &lines)
        }
        Some("del") => {
            let name = args
                .get(1)
                .ok_or_else(|| "использование: users del <name>".to_string())?;
            let lines = read_users(&path)?;
            let mut out = Vec::new();
            let mut found = false;
            for line in lines {
                let keep = line
                    .split_once(':')
                    .map_or(true, |(u, _)| !u.trim().eq_ignore_ascii_case(name));
                if !keep {
                    found = true;
                }
                if keep {
                    out.push(line);
                }
            }
            if !found {
                return Err(format!("пользователь '{}' не найден", name));
            }
            save_users(&path, &out)
        }
        _ => {
            print_help();
            Ok(())
        }
    }
}

// ---------------------------------------------------------------- start

fn run_openssl(args: &[&str]) -> Result<(), String> {
    let status = Command::new("openssl")
        .args(args)
        .stdout(Stdio::null())
        .status()
        .map_err(|e| format!("не удалось запустить openssl: {}", e))?;
    if !status.success() {
        return Err(format!("openssl завершился с кодом {:?}", status.code()));
    }
    Ok(())
}

fn cmd_start() -> Result<(), String> {
    let dir = script_dir();
    let geo = geo_dir(&dir);
    fs::create_dir_all(&geo).map_err(|e| e.to_string())?;

    let host = env_or_file("SERVER_IP", &dir)
        .ok_or_else(|| "SERVER_IP не задан (переменная окружения или server.env)".to_string())?;

    // 1. GeoIP/GeoSite — до всего остального
    for kind in ["geoip", "geosite"] {
        let path = geo.join(format!("{}.dat", kind));
        let missing = fs::metadata(&path)
            .map(|m| m.len() < 100_000)
            .unwrap_or(true);
        if missing {
            let base = env::var("GEO_BASE_URL").unwrap_or_else(|_| {
                "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download".into()
            });
            let cdn = env::var("GEO_CDN_BASE").unwrap_or_else(|_| {
                "https://cdn.jsdelivr.net/gh/Loyalsoldier/v2ray-rules-dat@release".into()
            });
            let explicit = env::var(if kind == "geoip" { "GEOIP_URL" } else { "GEOSITE_URL" }).ok();
            let urls = urls_for(&format!("{}.dat", kind), &base, &cdn, explicit);
            update_geo(kind, &geo, &urls)?;
        }
    }

    // 2. Сертификаты (как в базовой версии)
    for f in ["ca.crt", "server.crt", "server.csr", "extfile.cnf", "ca.srl"] {
        let _ = fs::remove_file(geo.join(f));
    }
    run_openssl(&["genrsa", "-out", "ca.key", "2048"])?;
    run_openssl(&[
        "req", "-new", "-x509", "-days", "3650", "-key", "ca.key", "-subj",
        "/CN=Hysteria Root CA", "-out", "ca.crt",
    ])?;
    run_openssl(&[
        "req", "-newkey", "rsa:2048", "-nodes", "-keyout", "server.key", "-subj",
        &format!("/CN={}", host), "-out", "server.csr",
    ])?;
    fs::write(geo.join("extfile.cnf"), format!("subjectAltName=IP:{}", host))
        .map_err(|e| e.to_string())?;
    run_openssl(&[
        "x509", "-req", "-days", "3650", "-in", "server.csr", "-CA", "ca.crt",
        "-CAkey", "ca.key", "-CAcreateserial", "-out", "server.crt", "-extfile",
        "extfile.cnf",
    ])?;

    // 3. users.txt
    let users = users_path(&dir);
    if !users.exists() {
        let mut f = fs::File::create(&users).map_err(|e| e.to_string())?;
        writeln!(f, "# username:password").map_err(|e| e.to_string())?;
        writeln!(f, "devuser1:iphusae2aigh7leem1ohshegh3oo").map_err(|e| e.to_string())?;
    }

    // 4. Запуск сервера
    let config = config_path(&dir);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = Command::new("hysteria")
            .arg("server")
            .arg("-c")
            .arg(&config)
            .exec();
        Err(format!("не удалось запустить hysteria: {}", err))
    }
    #[cfg(not(unix))]
    {
        let status = Command::new("hysteria")
            .arg("server")
            .arg("-c")
            .arg(&config)
            .status()
            .map_err(|e| format!("не удалось запустить hysteria: {}", e))?;
        process::exit(status.code().unwrap_or(1));
    }
}
