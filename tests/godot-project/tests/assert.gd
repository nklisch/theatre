## Assertion helpers for the GDScript test runner.
## Each method returns "" on success or a human-readable error string on failure.
class_name Assert


static func eq(actual: Variant, expected: Variant, label: String = "") -> String:
	if actual == expected:
		return ""
	return "expected %s == %s%s" % [str(expected), str(actual),
		" (%s)" % label if label else ""]


static func ne(actual: Variant, expected: Variant, label: String = "") -> String:
	if actual != expected:
		return ""
	return "expected not equal to %s%s" % [str(actual),
		" (%s)" % label if label else ""]


static func true_(val: bool, label: String = "") -> String:
	if val:
		return ""
	return "expected true%s" % [" (%s)" % label if label else ""]


static func false_(val: bool, label: String = "") -> String:
	if not val:
		return ""
	return "expected false%s" % [" (%s)" % label if label else ""]


static func not_null(val: Variant, label: String = "") -> String:
	if val != null:
		return ""
	return "expected non-null%s" % [" (%s)" % label if label else ""]


static func is_null(val: Variant, label: String = "") -> String:
	if val == null:
		return ""
	return "expected null, got %s%s" % [str(val), " (%s)" % label if label else ""]


static func approx(actual: float, expected: float,
		epsilon: float = 0.01, label: String = "") -> String:
	if absf(actual - expected) < epsilon:
		return ""
	return "expected ~%f got %f%s" % [expected, actual,
		" (%s)" % label if label else ""]


static func has_method(obj: Object, method: String) -> String:
	if obj != null and obj.has_method(method):
		return ""
	return "object missing method: %s" % method


static func has_signal(obj: Object, sig: String) -> String:
	if obj != null and obj.has_signal(sig):
		return ""
	return "object missing signal: %s" % sig
