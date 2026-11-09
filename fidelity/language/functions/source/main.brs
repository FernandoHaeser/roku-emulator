' Functions: recursion, defaults, typed returns, subs, function references.
sub Main()
    print "@@PROBE_BEGIN"
    print Factorial(10)
    print Greet("Ana"), Greet()
    print ToInteger(3.7), type(ToInteger(3.7))
    print ToFloat(3), type(ToFloat(3))
    r = NoReturn()
    print type(r)
    f = Factorial
    print type(f), f(5)
    square = function(v)
        return v * v
    end function
    print square(9)
    print Apply(square, 4)
    print "@@PROBE_END"
end sub

function Factorial(n as integer) as integer
    if n <= 1 then return 1
    return n * Factorial(n - 1)
end function

function Greet(name = "World" as string) as string
    return "Hi " + name
end function

function ToInteger(v as float) as integer
    return v
end function

function ToFloat(v as integer) as float
    return v
end function

sub NoReturn()
end sub

function Apply(fn as function, v as dynamic) as dynamic
    return fn(v)
end function
