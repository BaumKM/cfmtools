from cfmtools.util import JSON

class CFM:
    """
    Native representation of a cardinality-based feature model.

    Instances of this class are backed by a native (Rust) implementation and
    provide access to performance-critical operations.
    """

    @staticmethod
    def from_bytes(data: bytes) -> CFM:
        """
        Construct a native feature model from a serialized JSON representation.

        The input must be a UTF-8 encoded JSON object describing a
        cardinality-based feature model. The expected format includes the following
        top-level fields:

        - ``version`` (int): Format version identifier. Currently, only version
          ``1`` is supported.
        - ``feature_names`` (list[str]): Names of all features in the model.
        - ``root`` (str): Name of the root feature.
        - ``parents`` (dict[str, str]): Mapping from child feature names to their
          parent feature names.
        - ``feature_instance_cardinalities`` (dict[str, list[[int, int | null]]]):
          Feature instance cardinalities as lists of simple cardinality intervals.
        - ``group_instance_cardinalities`` (dict[str, list[[int, int | null]]]):
          Group instance cardinalities as lists of simple cardinality intervals.
        - ``group_type_cardinalities`` (dict[str, list[[int, int | null]]]):
          Group type cardinalities as lists of simple cardinality intervals.
        - ``require_constraints`` (list[object]): Require constraints, each
          specifying two features and their associated cardinalities.
        - ``exclude_constraints`` (list[object]): Exclude constraints with the
          same structure as require constraints.

        Simple cardinality intervals are represented as lists of intervals
        ``[lower, upper]``, where ``upper`` may be ``null`` to denote an
        unbounded interval.

        Returns:
            A native ``CFM`` instance backed by the Rust implementation.
        """
        ...

    def structural(self) -> StructuralCFM:
        """
        Obtain a structural configuration-space view of this feature model.

        This method does not modify the underlying feature model. Instead, it
        selects the structural interpretation of the configuration space and
        returns an object providing access to operations defined for that space,
        such as sampling and analysis.
        """
        ...

class StructuralCFM:
    def benchmark_ranking_sampler(
        self,
        runs: int,
        samples: int,
        seed: int,
        calculate_constrained_space_size: bool,
    ) -> JSON:
        """
        Benchmark the uniform ranking-based sampling algorithm.

        Executes multiple independent runs of the ranking sampler and returns
        aggregated benchmark results.
        """
        ...

    def benchmark_backtracking_sampler(
        self,
        runs: int,
        samples: int,
        seed: int,
        calculate_constrained_space_size: bool,
    ) -> JSON:
        """
        Benchmark the uniform backtracking-based sampling algorithm.

        Executes multiple independent runs of the backtracking sampler and
        returns aggregated benchmark results.
        """
        ...
    # --------------------------------------------------
    # Configuration space analysis
    # --------------------------------------------------

    def unconstrained_config_space_summary(
        self,
        show_full_tree: bool = False,
    ) -> JSON:
        """
        Compute a structural summary of the unconstrained configuration space.

        The summary includes:

        - Total number of unconstrained configurations
        - Average configuration size
        - Tree structure statistics
        - Dynamic-programming build times
        """
        ...

    def constrained_config_space_summary(
        self,
        time_limit_s: int,
        show_rank_validity: bool = False,
    ) -> JSON:
        """
        Compute a structural summary of the constrained configuration space,
        by enumerating unconstrained configurations.

        The summary includes:

        - Number of enumerated configurations
        - Number of valid configurations
        - Valid ratio
        - Average size of valid configurations
        - Time to first valid configuration
        - Enumeration status and timing information

        If enumeration does not finish within the time limit,
        statistics are based on the enumerated prefix only.
        """
        ...
