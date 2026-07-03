from __future__ import annotations

import platform
from pathlib import Path
from typing import Any

import yaml
from pydantic import BaseModel, ConfigDict, Field, PositiveInt, field_validator, model_validator


class StepConfig(BaseModel):
    name: str
    tool: str
    arguments: dict[str, Any] = Field(default_factory=dict)
    capture: dict[str, str] = Field(default_factory=dict)

    @field_validator("name", "tool")
    @classmethod
    def reject_blank(cls, value: str) -> str:
        if not value.strip():
            raise ValueError("value cannot be blank")
        return value


class ClientProfileConfig(BaseModel):
    name: str
    variables: dict[str, str] = Field(default_factory=dict)
    scenario: list[StepConfig] = Field(default_factory=list)

    @field_validator("name")
    @classmethod
    def reject_blank(cls, value: str) -> str:
        if not value.strip():
            raise ValueError("value cannot be blank")
        return value


class ClientConfig(BaseModel):
    count: PositiveInt | None = None
    list_tools_on_connect: bool
    profiles: list[ClientProfileConfig] = Field(default_factory=list)

    @model_validator(mode="after")
    def validate_profiles(self) -> ClientConfig:
        if not self.profiles and self.count is None:
            raise ValueError("clients.count is required when clients.profiles is empty")
        if self.profiles:
            names = [profile.name for profile in self.profiles]
            if len(names) != len(set(names)):
                raise ValueError("client profile names must be unique")
            if self.count is None:
                self.count = len(self.profiles)
            if self.count != len(self.profiles):
                raise ValueError("clients.count must match the number of client profiles")
        return self

    def resolved_count(self) -> int:
        if self.count is None:
            raise ValueError("client count has not been resolved")
        return self.count

    def profile_for(self, index: int) -> ClientProfileConfig | None:
        if not self.profiles:
            return None
        return self.profiles[index]


class PlatformCommand(BaseModel):
    windows: str
    unix: str

    def selected(self) -> str:
        if platform.system() == "Windows":
            return self.windows
        return self.unix


class ServerConfig(BaseModel):
    command: str | PlatformCommand
    args: list[str] = Field(default_factory=list)
    cwd: Path | None = None
    env: dict[str, str] = Field(default_factory=dict)

    def selected_command(self) -> str:
        if isinstance(self.command, PlatformCommand):
            return self.command.selected()
        return self.command


class ReportConfig(BaseModel):
    directory: Path
    summary_file: str
    events_file: str


class BenchConfig(BaseModel):
    model_config = ConfigDict(arbitrary_types_allowed=True)
    clients: ClientConfig
    server: ServerConfig
    scenario: list[StepConfig] = Field(default_factory=list)
    report: ReportConfig

    @model_validator(mode="after")
    def require_steps(self) -> BenchConfig:
        if not self.scenario and not self.clients.profiles:
            raise ValueError("scenario must contain at least one step")
        for profile in self.clients.profiles:
            if not profile.scenario and not self.scenario:
                raise ValueError(f"profile {profile.name} must contain at least one step")
        return self


def load_config(path: Path) -> BenchConfig:
    with path.open("r", encoding="utf-8") as file:
        data = yaml.safe_load(file)
    if not isinstance(data, dict):
        raise ValueError("configuration root must be a mapping")
    config = BenchConfig.model_validate(data)
    return resolve_config_paths(config, path.parent)


def resolve_config_paths(config: BenchConfig, base_dir: Path) -> BenchConfig:
    command = config.server.command
    if isinstance(command, PlatformCommand):
        command = PlatformCommand(
            windows=resolve_command(command.windows, base_dir),
            unix=resolve_command(command.unix, base_dir),
        )
    else:
        command = resolve_command(command, base_dir)
    cwd = resolve_path(config.server.cwd, base_dir)
    report = config.report.model_copy(
        update={"directory": resolve_path(config.report.directory, base_dir)}
    )
    server = config.server.model_copy(update={"command": command, "cwd": cwd})
    return config.model_copy(update={"server": server, "report": report})


def resolve_command(command: str, base_dir: Path) -> str:
    if has_path_syntax(command):
        return str((base_dir / command).resolve())
    return command


def resolve_path(path: Path | None, base_dir: Path) -> Path | None:
    if path is None or path.is_absolute():
        return path
    return (base_dir / path).resolve()


def has_path_syntax(command: str) -> bool:
    return "/" in command or "\\" in command or command.startswith(".")
