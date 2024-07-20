fn main() {
    #[cfg(esp32)]
    embuild::espidf::sysenv::output();
}
