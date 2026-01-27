fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("ac_pro_engineer.ico");
        {
            let this = res.compile();
            match this {
                Ok(t) => t,
                Err(e) => unwrap_failed("called `Result::unwrap()` on an `Err` value", &e),
            }
        };
    }
}

fn unwrap_failed(arg: &str, e: &std::io::Error) {
    todo!()
}
