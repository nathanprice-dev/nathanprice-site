+++
title = "Getting Started with IPv6 at Home"
date = "2026-02-28"
template = "page.html"
summary = "Reflections on using IPv6 at home to run a web server."
+++

Recently I become interested in working with IPv6 on my home network. I have a
Raspberry Pi that I wanted to host a simple web server and possibly some other
services. I became curious on whether I could host the server on only IPv6 and
not worry about using IPv4 at all. I figured this was a good excuse to learn
something new that would possibly be moderately useful.

I’ve worked with IPv4 in the past when working on networking between services
and applications. While I’m not as qualified as someone that specializes in
networking I feel relatively comfortable with my understanding of how networking
functions. I would like to share some of what I've learned by contrasting how I
expected things to work in IPv4 vs what I learned while getting a simple
solution to work in IPv6.

It's worth starting with some of the basics. An IPv4 address is a collection of
four eight bit integers. The common notation for representing these numbers is
as four integers between zero and 255 with dots between them i.e. 192.168.0.1.

IPv6 on the other hand consists of eight sixteen bit numbers. The common
notation for representing these numbers is as eight hexadecimal numbers between
0000 through FFFF with colons between them i.e.
AAAA:0000:0000:0000:0000:1234:0000:1111. That is a lot to write down so there
is a convention to shorten the address by using two colons to remove repeated
blocks of zeros ie. AAAA::1234:0000:1111. RFC 5952 has more details of how this
works if you would like to read more.

The biggest reason for introducing IPv6 is the increased number of addresses
that can be represented. IPv4 has a maximum of 2^32 addresses and IPv6 has the
maximum of 2^128 addresses. This added addresses makes it possible to connect
many more devices to the internet as well as making it more likely that
addresses blocks are assigned to the same geographical location which can
improve routing performance.

My understanding is that IPv4 was intended to be a system that would give each
device on the internet its own address. The wide popularity and broadened use
cases for the internet made that impractical with the number of possible
addresses in IPv4. To make it possible to have more servers on the internet
without each needing a public routable address NAT(Network Address translation)
was invented. This made it so that there could be one public address that
represented a much larger amount of private hosts. The router would then be
responsible to make sure that traffic that came to the public IP address would
get to the right device. 

IPv6 fixes this by having enough addresses that it is practical for every
device on the internet to have its own unique address. I had a couple concerns
as I started to think about this. The first issue was a concern for privacy. If
your device always has the same IP address on the Internet then it becomes
easier for advertisers or anyone else to build a profile of your activities
online. My other concern was around security since NAT makes it harder for
anyone the public Internet to try to connect to a computer with an internal
address directly. After looking into it further I found that there are a couple
of solutions that are in place to fix both of these concerns.

With IPv4 it is common to do automatic IP address assignment through DHCP
(dynamic host configuration protocol). IPv6 also supports DHCP but it also supports a
protocol called SLAAC (stateless address autoconfiguration). SLAAC like DHCP
will assign an ID to a device but SLAAC also has a privacy extension that can
periodically change that ID.  When the ID is changed, the last known ID of that
device continues to route to the device for a set amount of time making sure
that there isn't any service disruption during the process. 

Regarding the security concern, I realized that there is still a router between
your device and the public Internet. That device can provide security similar
to the old NAT system by setting firewall rules that only allows certain
traffic to be initiated to certain devices. This makes it so that even if
somebody does try to uniquely address your device with an unsolicited request,
the firewall will block it.

Like IPV4 IPv6 also has subnets. Depending on your ISP you may be able to
get a static address block/subnet assigned to you by your ISP. For my project I
was able to request and get my own block of addresses. These address blocks can
come in a number of sizes such as /48 or /56. On the Ubiquiti system I have at
home each vnet gets assigned a /64 address. Below is a table of how many
addresses exist in each of these subnets to give you an idea of how many
devices can be supported based on your subnet size.

| subnet size | number of devices                 | 
| /48         | 1,208,925,819,614,629,174,706,176 |
| /56         |     4,722,366,482,869,645,213,696 |
| /64         |         18,446,744,073,709,551,616|

I also found that working with IPv6 addresses was pretty straightforward as
well. For example, you can run the following curl command to hit an IPv6
address directly `curl -6 http://[0000:000::1]/` or `curl -6
http://www.example.com`. A similar approach can be used to address an IPv6
address directly from the web browser but remember that you need to use square
brackets to surround the address so that it recognizes it as IP address. These
commands are very helpful when trying to set up and test my IPv6 server. Many
of the commands that are commonly used when working when debugging and testing
networks also work with IPv6 with slight changes. There is a ping6 command to
let you ping over IPv6. Other commands just involve adding a -6 to them.  Tools
like Wireshark are also functional and continue to be very useful.

On the software side I found that things are getting pretty well supported for
IPv6 I run a UniFi router and found that their support for IPv6 is OK. It took
me some effort to get the prefix delegation set up and to properly assign
static addresses, but it worked well once I figured out the correct way to get
it configured. I wish that it gave me more control over how I laid out my
network and address assignments, but was glad that the solution worked. I found
that most of my devices support dual stack which means that they can
communicate over IPv6 and IPv4 at the same time.

Ultimately I was able to get my Web server up and running and was able to have
it use only IPv6. I was able to access it from my cell phone carrier, but I
found that other networks I connected to still don’t have a good IPv6 support
so I was somewhat limited in the places where I could reach this server. Since
I was just serving static content I was able to use Cloudflare as a proxy which
transparently gave me IPv4 support without having to use IPv6 on my server.
Understandably this isn’t something that you would do in a production
environment, but it was fun to learn something new and have a working solution
at the end.
