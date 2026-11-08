// Copyright (c) 2024 Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0
#include <algorithm>
#include <cstdint>
#include <iomanip>
#include <iostream>
#include <memory>
#include <layerprocgen/common/random_hash.hpp>
#include <layerprocgen/common/fixed.hpp>
#include <layerprocgen/samples/simple_layers.hpp>
#include <layerprocgen/save_state/save_state.hpp>
using namespace runevision::common;
using namespace runevision::layerprocgen::samples;
int main() {
    for (std::uint32_t seed : {0U,1U,1234U,0xffffffffU})
        for (std::uint32_t value : {0U,1U,9U,0xfffffff7U,0xffffffffU})
            std::cout << "hash " << seed << ' ' << value << ' ' << RandomHash(seed).hash(value) << '\n';
    for (std::int32_t x=-4;x<=4;++x)
        for (std::int32_t y=-4;y<=4;++y)
            for (std::uint32_t salt=0;salt<6;++salt)
                std::cout << "multi " << x << ' ' << y << ' ' << salt << ' ' << RandomHash(1234U).hash({static_cast<std::uint32_t>(x),static_cast<std::uint32_t>(y),salt}) << '\n';
    for (double number : {-12.75,-1.1,-0.5,-0.001,0.0,0.1,0.5,1.1,100.999}) {
        Fixed8 value(number);
        std::cout << "fixed " << value.raw() << ' ' << value.floorToInt() << ' ' << value.ceilToInt() << ' ' << value.roundToInt() << '\n';
    }
    PointsLayer layer;
    auto request=layer.request({{-1024,-1024},{1280,1280}},0);
    auto points=layer.pointsInBounds({{-1024,-1024},{1280,1280}});
    std::ranges::sort(points);
    for (auto point:points) std::cout << "point " << point.x << ' ' << point.y << '\n';
    request.release(); layer.shutdown();
    runevision::save_state::SaveState save;
    save.set_bool_values(-9,{true,false,true});
    save.set_bool_values(12,{});
    save.set_int_values(-1,{-2147483647-1,0,2147483647});
    save.set_point_values(7,{{-300,256},{92,167}});
    save.set_point3_values(0,{{-1,2,-3}});
    auto bytes=save.to_bytes();
    if (!bytes) return 1;
    std::cout << "save ";
    for(auto byte:bytes.value()) std::cout << std::hex << std::setfill('0') << std::setw(2) << static_cast<unsigned>(byte);
    std::cout << '\n';
}
