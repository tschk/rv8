```markdown
# rv8 Development Patterns

> Auto-generated skill from repository analysis

## Overview
This skill teaches best practices and conventions for contributing to the `rv8` Rust codebase. You'll learn how to structure files, write imports and exports, and follow the project's conventions for naming and testing. While no specific frameworks or automated workflows are detected, this guide provides clear instructions and commands to help you work efficiently within the repository.

## Coding Conventions

### File Naming
- Use **snake_case** for all file and module names.
  - Example:  
    ```rust
    // Good
    mod cpu_core;
    // Bad
    mod CpuCore;
    ```

### Import Style
- Use **relative imports** within the crate.
  - Example:
    ```rust
    use crate::cpu_core::registers;
    use super::utils;
    ```

### Export Style
- Use **named exports** to expose items.
  - Example:
    ```rust
    pub mod cpu_core;
    pub fn run() { /* ... */ }
    ```

### Commit Patterns
- Commit messages are freeform, often descriptive, and average about 98 characters.
  - Example:
    ```
    Fix memory alignment issue in load/store instructions
    ```

## Workflows

### Adding a New Module
**Trigger:** When creating a new feature or logical component  
**Command:** `/add-module`

1. Create a new file using snake_case (e.g., `my_feature.rs`).
2. Define your module and its public interface.
3. Use relative imports for dependencies.
4. Export the module in the parent `mod.rs` or `lib.rs`.
5. Write corresponding tests in a `my_feature.test.rs` file.

### Running Tests
**Trigger:** When verifying code correctness  
**Command:** `/run-tests`

1. Locate test files matching the `*.test.*` pattern.
2. Use Rust's built-in test runner:
    ```sh
    cargo test
    ```
3. Review test output and address any failures.

### Making a Commit
**Trigger:** After completing a logical change  
**Command:** `/commit-change`

1. Stage your changes:
    ```sh
    git add .
    ```
2. Write a descriptive commit message (freeform, ~98 chars).
3. Commit your changes:
    ```sh
    git commit -m "Describe your change here"
    ```

## Testing Patterns

- Test files follow the pattern `*.test.*` (e.g., `cpu_core.test.rs`).
- The testing framework is not explicitly specified, but Rust's built-in test framework is likely used.
- Example test structure:
    ```rust
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_add() {
            assert_eq!(add(2, 2), 4);
        }
    }
    ```

## Commands

| Command         | Purpose                                   |
|-----------------|-------------------------------------------|
| /add-module     | Scaffold a new module with conventions    |
| /run-tests      | Run all tests in the repository           |
| /commit-change  | Commit staged changes with a message      |
```
