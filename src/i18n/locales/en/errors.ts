export const errors = {
  errors: {
    io: "I/O error",
    database: "Database error",
    network: "Network error",
    keyring: "Credentials vault error",
    serialization: "Serialization error",
    auth: "Authentication error",
    emulator_not_detected: "Emulator not recognized in folder",
    emulator_exists: "An emulator with this name already exists",
    file_busy: "File in use (modified while reading)",
    unexpected: "Unexpected error talking to the backend",
  },
} as const;
