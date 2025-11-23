/*==============================================================================================================================

   Copyright © 2003 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : "Common"
   Description          : Common Configuration file

   Author               : RTT
   Version              : 1.0
   Creation Date        : 5/27/2003

================================================================================================================================
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================
   Notes for Use and Synthesis
==============================================================================================================================*/

`ifdef   RELATIVE_FILENAMES

`include "A2_defines.vh"
`include "A2Sv2r3_defines.vh"
`include "A2Sv2r3_Registers.vh"
`include "NEWPORT_defines.vh"
`else
`include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_defines.vh"
`include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2Sv2r3_defines.vh"
`include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2Sv2r3_Registers.vh"
`include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/NEWPORT_defines.vh"
`endif


// Let's assume we are in the Gigahertz range and set our base clock at 1 GHz    1     ns
// We also have a fast clock that is running 4 times faster or          4 GHz    0.25  ns
// So lets assume we need 10 timesteps per cycle                                 0.025 ns
// Nearest timescale that suits is therefore:

`timescale  1ps/10fs //JLPL - 9_5_25

// So our fastest clock should have 10 time units per period
// Therefore 'fast' clock is  10 time units and a
// and       'slow' clock is  40 time units

`define  S(x)     $signed(x)

`define  A2S_CLOCK_PERIOD         40
`define  FAST_CLOCK_PERIOD        10
`define  tCQ                      #4
`define  tFCQ                     #1
`define  tACC                    #20
`define  CM_NUM_CLOCKS_READ        1
`define  CM_NUM_CLOCKS_CYCLE       1
`define  DM_NUM_CLOCKS_READ        1
`define  DM_NUM_CLOCKS_CYCLE       1
`define  WM_NUM_CLOCKS_READ        1
`define  WM_NUM_CLOCKS_CYCLE       1

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
