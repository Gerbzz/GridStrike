"""Negative controls for the packaged-icon CI gate; no app execution required."""
import struct
import unittest
from verify_geometric_identity import validate_group


class IconGroupCoverage(unittest.TestCase):
    def setUp(self):
        self.sizes = [16, 24, 32, 48, 64, 128, 256]
        self.images = {size: bytes([index+1]) * (index+4) for index, size in enumerate(self.sizes)}
        self.resources = {index+1: self.images[size] for index, size in enumerate(self.sizes)}

    def group(self, sizes):
        return struct.pack("<HHH", 0, 1, 7) + b"".join(
            struct.pack("<BBBBHHIH", size % 256, size % 256, 0, 0, 1, 32, len(self.images[size]), self.sizes.index(size)+1)
            for size in sizes
        )

    def resource(self, resource_id, kind):
        self.assertEqual(kind, 3)
        return self.resources[resource_id]

    def test_all_seven_sizes_pass(self):
        self.assertEqual(validate_group(self.group(self.sizes), self.resource, self.images), self.sizes)

    def test_repeated_small_icon_is_rejected(self):
        with self.assertRaisesRegex(AssertionError, "size coverage"):
            validate_group(self.group([16]*7), self.resource, self.images)

    def test_wrong_declared_payload_size_is_rejected(self):
        group = bytearray(self.group(self.sizes))
        struct.pack_into("<I", group, 6+8, 1)
        with self.assertRaisesRegex(AssertionError, "payload size"):
            validate_group(group, self.resource, self.images)


if __name__ == "__main__":
    unittest.main()
