    ///initialize system logger
    pub fn init(log_root: &str, t_log: String, with_trace: bool) -> Result<(),String>{
        let log_dir = log_root.to_string() + "/log";
        std::fs::create_dir(&log_dir).ok();
        let target1 = t_log.to_string();
        let t_trace = t_log.to_string();
        let fern = fern::DateBased::new("log/",format!("{}-error-%Y%m%d.log",t_log.to_lowercase()));
        let error = fern::Dispatch::new()
        .format(move |out, message, _| {
            out.finish(format_args!(
                "[{}] {}",
                target1,
                message
            ))
        })
        .level(log::LevelFilter::Warn)
        .filter(|metadata| metadata.level() == log::LevelFilter::Warn || metadata.level() == log::LevelFilter::Error)
        .chain(fern);



        let fern = fern::DateBased::new("log/",format!("{}-log-%Y%m%d.log",t_log.to_lowercase()));
        let info = fern::Dispatch::new()
        .format(move |out, message, _| {
            let msg = format_args!(
                "[{}] {}",
                t_log,
                message
            ).to_string().replace("unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 }", "");
            out.finish(format_args!("{}", msg))
        })
        .level(log::LevelFilter::Info)
        .filter(|metadata| metadata.level() == log::LevelFilter::Info || metadata.level() == log::LevelFilter::Error)
        .chain(fern);

        let mut dis = fern::Dispatch::new()
        .format( |out, message, record| {
            out.finish(format_args!(
                "-[{}][{}]{} {}",
                record.target(),
                record.level(),
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                message
            ))
        })
        .chain(error).chain(info);
        if with_trace {
            let fern = fern::DateBased::new("log/",format!("{}-trace-%Y%m%d.log",t_trace.to_lowercase()));
            let trace = fern::Dispatch::new()
            .format(move |out, message, _| {
                out.finish(format_args!(
                    "[{}] {}",
                    t_trace,
                    message
                ))
            })
            .level(log::LevelFilter::Trace)
            .filter(|metadata| metadata.level() == log::LevelFilter::Trace)
            .chain(fern);
            dis = dis.chain(trace);
        }
        dis.apply().expect("fail to initialize logger");
        std::panic::set_hook(Box::new(|info|{
            error!("system panic {:?}",info);
        }));
        info!("logger 初始化成功");
        Ok(())
    }