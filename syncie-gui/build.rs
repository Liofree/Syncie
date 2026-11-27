// Icon der .exe (Explorer, Verknuepfung, angeheftete Taskleiste). Das
// FENSTER-Icon steht in main.rs - Windows fragt beides getrennt ab.
// Schlaegt das Einbetten fehl (fehlendes SDK), bleibt es eine Warnung:
// ein Bild darf keinen Build kosten.
fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=syncie.ico");
        if let Err(e) = winresource::WindowsResource::new()
            .set_icon("syncie.ico")
            .compile()
        {
            println!("cargo:warning=Icon nicht eingebettet: {e}");
        }
    }
}
