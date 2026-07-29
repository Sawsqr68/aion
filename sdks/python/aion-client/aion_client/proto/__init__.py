"""Generated AW workflow API stubs (workflow.proto + common.proto).

Every file in this directory except this one is protoc output and must not be
hand-edited. Regenerate from the shared wire contract with::

    cd sdks/python/aion-client && uv run --extra dev python -m proto_codegen

then commit the result. `proto_codegen.py` owns the exact protoc invocation and
the package-relative import rewrite, so the ritual is a command rather than a
recipe to retype — a recipe is what let these stubs fall a full contract behind
the server without a single test going red.

The ritual is not the safeguard, though; `tests/test_proto_contract.py` is. It
regenerates into a scratch directory and byte-compares against what is
committed, so drift fails CI instead of waiting to be noticed. That test is also
why this file is named in `proto_codegen.HAND_WRITTEN_FILES`: it is the one file
here regeneration cannot check, so it is checked behaviourally instead — every
generated module in this directory must be imported and re-exported below.
"""

from . import common_pb2, common_pb2_grpc, workflow_pb2, workflow_pb2_grpc

__all__ = ["common_pb2", "common_pb2_grpc", "workflow_pb2", "workflow_pb2_grpc"]
