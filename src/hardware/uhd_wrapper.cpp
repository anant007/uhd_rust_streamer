#include "uhd_wrapper.h"
#include <uhd/rfnoc_graph.hpp>
#include <uhd/rfnoc/mb_controller.hpp>
#include <uhd/types/stream_cmd.hpp>
#include <uhd/exception.hpp>
#include "/home/tsianant/uhd_rust_streamer/target/cxxbridge/rust/cxx.h"

namespace rfnoc_tool {

// RfnocGraphWrapper implementation
RfnocGraphWrapper::RfnocGraphWrapper(uhd::rfnoc::rfnoc_graph::sptr graph)
    : graph_(graph) {}

RfnocGraphWrapper::~RfnocGraphWrapper() = default;

rust::Vec<rust::String> RfnocGraphWrapper::get_block_ids() const {
    rust::Vec<rust::String> result;
    auto block_ids = graph_->find_blocks("");
    for (const auto& id : block_ids) {
        result.push_back(rust::String(id.to_string()));
    }
    return result;
}

std::unique_ptr<BlockControlWrapper> RfnocGraphWrapper::get_block(rust::Str block_id) const {
    std::string block_id_str(block_id.data(), block_id.size());// Was failing due to incorrect conversion from rust string to std::string, seems fixed now
    uhd::rfnoc::block_id_t id(block_id_str);
    // Use get_block<noc_block_base> instead of get_block with block_id
    auto block = graph_->get_block<uhd::rfnoc::noc_block_base>(id);
    if (!block) {
        std::string error_msg = "Block not found: " + block_id_str;
        throw rust::Error(error_msg); //Used string literal to get rid of rust::String conversion/initialisation error
    }
    return std::make_unique<BlockControlWrapper>(block);
}

void RfnocGraphWrapper::connect_blocks(rust::Str src_block, size_t src_port,
                                       rust::Str dst_block, size_t dst_port) {
    graph_->connect(std::string(src_block), src_port, 
                    std::string(dst_block), dst_port);
}

rust::Vec<GraphEdgeFFI> RfnocGraphWrapper::enumerate_connections() const {
    rust::Vec<GraphEdgeFFI> result;
    auto edges = graph_->enumerate_active_connections();
    for (const auto& edge : edges) {
        GraphEdgeFFI ffi_edge;
        ffi_edge.src_block_id = rust::String(edge.src_blockid);
        ffi_edge.src_port = edge.src_port;
        ffi_edge.dst_block_id = rust::String(edge.dst_blockid);
        ffi_edge.dst_port = edge.dst_port;
        result.push_back(ffi_edge);
    }
    return result;
}

void RfnocGraphWrapper::commit_graph() {
    graph_->commit();
}

std::unique_ptr<RxStreamerWrapper> RfnocGraphWrapper::create_rx_streamer(const StreamArgs& stream_args) {
    uhd::stream_args_t args(
        std::string(stream_args.cpu_format),
        std::string(stream_args.otw_format)
    );
    
    for (size_t chan : stream_args.channels) {
        args.channels.push_back(chan);
    }
    
    if (!stream_args.args.empty()) {
        args.args = uhd::device_addr_t(std::string(stream_args.args));
    }
    
    auto streamer = graph_->create_rx_streamer(args.channels.size(), args);
    return std::make_unique<RxStreamerWrapper>(streamer);
}

void RfnocGraphWrapper::connect_rx_streamer(const RxStreamerWrapper& streamer,
                                            rust::Str block_id, size_t port) {
    // Connect using the streamer's get_streamer() method
    graph_->connect(std::string(block_id), port, streamer.get_streamer(), 0);
}

double RfnocGraphWrapper::get_tick_rate() const {
    // Get tick rate from the first radio block or use default
    auto radio_blocks = graph_->find_blocks("Radio");
    if (!radio_blocks.empty()) {
        auto radio = graph_->get_block<uhd::rfnoc::noc_block_base>(radio_blocks[0]);
        return radio->get_tick_rate();
    }
    // Return default tick rate if no radio blocks found
    return 200e6; // 200 MHz default
}

void RfnocGraphWrapper::set_time_next_pps(const TimeSpec& time_spec) {
    uhd::time_spec_t ts(time_spec.secs, time_spec.nsecs / 1e9);
    // Use mb_controller index 0
    graph_->get_mb_controller(0)->get_timekeeper(0)->set_time_next_pps(ts);
}

TimeSpec RfnocGraphWrapper::get_time_now() const {
    auto ts = graph_->get_mb_controller(0)->get_timekeeper(0)->get_time_now();
    TimeSpec result;
    result.secs = ts.get_full_secs();
    result.nsecs = static_cast<uint32_t>(ts.get_frac_secs() * 1e9);
    return result;
}

// RxStreamerWrapper implementation
RxStreamerWrapper::RxStreamerWrapper(uhd::rx_streamer::sptr streamer)
    : streamer_(streamer) {}

RxStreamerWrapper::~RxStreamerWrapper() = default;

uhd::rx_streamer::sptr RxStreamerWrapper::get_streamer() const {
    return streamer_;
}

size_t RxStreamerWrapper::recv_samples(rust::Slice<uint8_t> buffs, size_t nsamps_per_buff,
                                       RxMetadata& metadata, double timeout) {
    std::vector<void*> buff_ptrs = {buffs.data()};
    uhd::rx_metadata_t md;
    
    size_t num_rx = streamer_->recv(buff_ptrs, nsamps_per_buff, md, timeout);
    
    metadata.has_time_spec = md.has_time_spec;
    if (md.has_time_spec) {
        metadata.time_spec.secs = md.time_spec.get_full_secs();
        metadata.time_spec.nsecs = static_cast<uint32_t>(md.time_spec.get_frac_secs() * 1e9);
    }
    metadata.more_fragments = md.more_fragments;
    metadata.fragment_offset = md.fragment_offset;
    metadata.start_of_burst = md.start_of_burst;
    metadata.end_of_burst = md.end_of_burst;
    metadata.error_code = static_cast<uint32_t>(md.error_code);
    
    return num_rx;
}

void RxStreamerWrapper::issue_stream_cmd(uint32_t stream_mode, uint64_t num_samps,
                                         bool stream_now) {
    uhd::stream_cmd_t cmd(static_cast<uhd::stream_cmd_t::stream_mode_t>(stream_mode));
    cmd.num_samps = num_samps;
    cmd.stream_now = stream_now;
    streamer_->issue_stream_cmd(cmd);
}

void RxStreamerWrapper::issue_stream_cmd_timed(uint32_t stream_mode, uint64_t num_samps,
                                               uint64_t time_secs, uint32_t time_nsecs,
                                               bool stream_now) {
    uhd::stream_cmd_t cmd(static_cast<uhd::stream_cmd_t::stream_mode_t>(stream_mode));
    cmd.num_samps = num_samps;
    cmd.stream_now = stream_now;
    if (!stream_now) {
        cmd.time_spec = uhd::time_spec_t(time_secs, time_nsecs / 1e9);
    }
    streamer_->issue_stream_cmd(cmd);
}

// BlockControlWrapper implementation
BlockControlWrapper::BlockControlWrapper(uhd::rfnoc::noc_block_base::sptr block)
    : block_(block) {}

BlockControlWrapper::~BlockControlWrapper() = default;

rust::String BlockControlWrapper::get_block_type() const {
    return rust::String(block_->get_block_id().get_block_name());
}

size_t BlockControlWrapper::get_num_input_ports() const {
    return block_->get_num_input_ports();
}

size_t BlockControlWrapper::get_num_output_ports() const {
    return block_->get_num_output_ports();
}

rust::Vec<rust::String> BlockControlWrapper::get_property_names() const {
    rust::Vec<rust::String> result;
    auto props = block_->get_property_ids();
    for (const auto& prop : props) {
        result.push_back(rust::String(prop));
    }
    return result;
}

void BlockControlWrapper::set_property_double(rust::Str name, double value, size_t port) {
    block_->set_property<double>(std::string(name), value, port);
}

double BlockControlWrapper::get_property_double(rust::Str name, size_t port) const {
    return block_->get_property<double>(std::string(name), port);
}

// Factory function
std::unique_ptr<RfnocGraphWrapper> create_rfnoc_graph(const DeviceArgs& args) {
    try {
        auto graph = uhd::rfnoc::rfnoc_graph::make(std::string(args.args));
        return std::make_unique<RfnocGraphWrapper>(graph);
    } catch (const uhd::exception& e) {
        throw rust::Error(rust::String(e.what()));
    }
}

} // namespace rfnoc_tool