/// @docs: [auth-login]
function login(username: string, password: string): boolean {
    // Authentication logic
    return true;
}

/// @docs: [user-create]
export function createUser(name: string, email: string): User {
    return new User(name, email);
}

function helperFunction(): void {
    // No docs annotation
}
