#pragma once

#include <memory>
#include <vector>
#include <string>
#include <cstdint>
#include <uhd/rfnoc_graph.hpp>
#include <uhd/rfnoc/block_id.hpp>
#include <uhd/rfnoc/noc_block_base.hpp>
#include <uhd/stream.hpp>
<<<<<<< HEAD
#include "rust/cxx.h"
=======
#include "target/cxxbridge/rust/cxx.h"
>>>>>>> dd8dc7e (Latest changes committed, still unsure as to what the core problems are)

namespace rfnoc_tool {

// Forward declarations
class RfnocGraphWrapper;
class RxStreamerWrapper;
class BlockControlWrapper;

// FFI-compatible structures (matching Rust definitions)
struct StreamArgs {
    rust::String cpu_format;
    rust::String otw_format;
    rust::Vec<size_t> channels;
    rust::String args;
};

struct DeviceArgs {
    rust::String args;
};

struct TimeSpec {
    uint64_t secs;
    uint32_t nsecs;
};

struct RxMetadata {
    bool has_time_spec;
    TimeSpec time_spec;
    bool more_fragments;
    size_t fragment_offset;
    bool start_of_burst;
    bool end_of_burst;
    uint32_t error_code;
};

struct GraphEdgeFFI {
    rust::String src_block_id;
    size_t src_port;
    rust::String dst_block_id;
    size_t dst_port;
};

// Wrapper classes
class RfnocGraphWrapper {
public:
    RfnocGraphWrapper(uhd::rfnoc::rfnoc_graph::sptr graph);
    ~RfnocGraphWrapper();

    rust::Vec<rust::String> get_block_ids() const;
    std::unique_ptr<BlockControlWrapper> get_block(rust::Str block_id) const;
    void connect_blocks(rust::Str src_block, size_t src_port,
                       rust::Str dst_block, size_t dst_port);
    rust::Vec<GraphEdgeFFI> enumerate_connections() const;
    void commit_graph();
    std::unique_ptr<RxStreamerWrapper> create_rx_streamer(const StreamArgs& stream_args);
    void connect_rx_streamer(const RxStreamerWrapper& streamer,
                            rust::Str block_id, size_t port);
    double get_tick_rate() const;
    void set_time_next_pps(const TimeSpec& time_spec);
    TimeSpec get_time_now() const;

private:
    uhd::rfnoc::rfnoc_graph::sptr graph_;
};

class RxStreamerWrapper {
public:
    RxStreamerWrapper(uhd::rx_streamer::sptr streamer);
    ~RxStreamerWrapper();

    size_t recv_samples(rust::Slice<uint8_t> buffs, size_t nsamps_per_buff,
                       RxMetadata& metadata, double timeout);
    void issue_stream_cmd(uint32_t stream_mode, uint64_t num_samps,
                         bool stream_now);
    void issue_stream_cmd_timed(uint32_t stream_mode, uint64_t num_samps,
                               uint64_t time_secs, uint32_t time_nsecs,
                               bool stream_now);
    
    // Add getter for streamer
    uhd::rx_streamer::sptr get_streamer() const;

private:
    uhd::rx_streamer::sptr streamer_;
};

class BlockControlWrapper {
public:
    BlockControlWrapper(uhd::rfnoc::noc_block_base::sptr block);
    ~BlockControlWrapper();

    rust::String get_block_type() const;
    size_t get_num_input_ports() const;
    size_t get_num_output_ports() const;
    rust::Vec<rust::String> get_property_names() const;
    void set_property_double(rust::Str name, double value, size_t port);
    double get_property_double(rust::Str name, size_t port) const;

private:
    uhd::rfnoc::noc_block_base::sptr block_;
};

// Factory function
std::unique_ptr<RfnocGraphWrapper> create_rfnoc_graph(const DeviceArgs& args);

} // namespace rfnoc_tool