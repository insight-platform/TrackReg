from typing import List, Optional, Dict


class CounterFamily:

    @classmethod
    def get_or_create_counter_family(
            cls,
            name: str,
            description: Optional[str],
            label_names: List[str],
            unit: Optional[str],
    ) -> CounterFamily: ...

    @classmethod
    def get_counter_family(cls, name: str) -> Optional[CounterFamily]: ...

    def set(self, value: int, label_values: List[str]) -> int: ...

    def inc(self, value: int, label_values: List[str]) -> int: ...

    def delete(self, label_values: List[str]) -> Optional[int]: ...

    def get(self, label_values: List[str]) -> Optional[int]: ...


class GaugeFamily:
    @classmethod
    def get_or_create_gauge_family(
            cls,
            name: str,
            description: Optional[str],
            label_names: List[str],
            unit: Optional[str],
    ) -> GaugeFamily: ...

    @classmethod
    def get_gauge_family(cls, name: str) -> Optional[GaugeFamily]: ...

    def set(self, value: float, label_values: List[str]) -> float: ...

    def delete(self, label_values: List[str]) -> Optional[float]: ...

    def get(self, label_values: List[str]) -> Optional[float]: ...


def delete_metric_family(name: str) -> None: ...


def set_extra_labels(labels: Dict[str, str]) -> None: ...
